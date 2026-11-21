//! The library's behaviour, end to end through the desks, the stores and the relays.

use bootstrap::Library;
use catalogue_adapters::incoming::desk::DeskResponse as CatalogueResponse;
use lending_adapters::incoming::desk::DeskResponse as LendingResponse;

const JANE: u128 = 1_001;
const JOHN: u128 = 1_002;
const SUSPENDED: u128 = 1_003;

fn created(response: CatalogueResponse) -> u128 {
    match response {
        CatalogueResponse::Created { id, .. } => id,
        other => panic!("expected an entry, got {other:?}"),
    }
}

fn done(response: LendingResponse) -> u128 {
    match response {
        LendingResponse::Done { id } => id,
        other => panic!("expected done, got {other:?}"),
    }
}

/// A library with a title and one copy of it, shelved in lending.
async fn with_copy() -> (Library, u128) {
    let library = Library::open(&[SUSPENDED]);
    let title = created(library.catalogue.register_title("Dune").await);
    let copy = created(library.catalogue.acquire_copy(title).await);
    library.relay().await.unwrap();
    (library, copy)
}

#[tokio::test]
async fn a_title_registered_and_a_copy_acquired_are_held_by_the_catalogue() {
    let library = Library::open(&[]);
    let title = created(
        library
            .catalogue
            .register_title("  The Left Hand of Darkness ")
            .await,
    );
    assert!(library.title_held(title).await);

    let acquired = library.catalogue.acquire_copy(title).await;
    let CatalogueResponse::Created { id, shelf_mark } = acquired else {
        panic!("expected a copy, got {acquired:?}");
    };
    assert_eq!(shelf_mark, Some(format!("THE-{id}")));
    assert_eq!(library.holdings_of(title).await, Some(1));

    assert_eq!(
        library
            .catalogue
            .register_title("The Left Hand of Darkness")
            .await,
        CatalogueResponse::Rejected {
            reason: "name-taken"
        }
    );
    assert_eq!(
        library.catalogue.register_title(" ").await,
        CatalogueResponse::Rejected {
            reason: "blank-name"
        }
    );
    assert_eq!(
        library.catalogue.acquire_copy(424_242).await,
        CatalogueResponse::NotFound
    );
}

#[tokio::test]
async fn the_copy_and_its_delivery_to_lending_are_committed_together() {
    let library = Library::open(&[]);
    let title = created(library.catalogue.register_title("Dune").await);
    assert_eq!(library.catalogue_deliveries_pending().await, 0);

    let copy = created(library.catalogue.acquire_copy(title).await);

    assert_eq!(library.holdings_of(title).await, Some(1));
    assert_eq!(library.catalogue_deliveries_pending().await, 1);
    assert_eq!(
        library.on_shelf(copy).await,
        None,
        "lending learns of it only after the relay"
    );
}

#[tokio::test]
async fn the_relay_delivers_after_the_commit_to_the_listener_in_lending() {
    let library = Library::open(&[]);
    let title = created(library.catalogue.register_title("Dune").await);
    let copy = created(library.catalogue.acquire_copy(title).await);

    assert_eq!(
        library.relay().await.unwrap(),
        2,
        "the copy acquired, then the copy shelved"
    );

    assert_eq!(library.on_shelf(copy).await, Some(true));
    assert_eq!(library.catalogue_deliveries_pending().await, 0);
    assert_eq!(library.deliveries_received().await, 1);
    assert_eq!(
        library.relay().await.unwrap(),
        0,
        "nothing is delivered twice by a relay that recorded its deliveries"
    );
}

#[tokio::test]
async fn a_repeated_delivery_is_passed_over_by_the_inbox() {
    let (library, copy) = with_copy().await;
    done(library.lending.lend(JANE, copy, None).await);
    library.relay().await.unwrap();

    library.lose_relay_acknowledgements().await;
    let redelivered = library.relay().await.unwrap();

    assert!(redelivered >= 1, "the catalogue's event arrived again");
    assert_eq!(
        library.on_shelf(copy).await,
        Some(false),
        "the copy lent was not shelved anew"
    );
    assert_eq!(library.loans_of(copy).await, 1);
    assert_eq!(library.deliveries_received().await, 1);
}

#[tokio::test]
async fn a_member_borrows_a_copy_and_the_reservation_they_collect_is_fulfilled_after_the_commit() {
    let (library, copy) = with_copy().await;
    let reservation = done(library.lending.reserve(JANE, copy).await);

    done(library.lending.lend(JANE, copy, Some(reservation)).await);
    assert_eq!(library.on_shelf(copy).await, Some(false));
    assert_eq!(
        library.reservation_fulfilled(reservation).await,
        Some(false)
    );

    library.relay().await.unwrap();
    assert_eq!(library.reservation_fulfilled(reservation).await, Some(true));

    assert_eq!(
        library.lending.lend(JOHN, copy, None).await,
        LendingResponse::Refused {
            reason: "not-on-shelf"
        }
    );
}

#[tokio::test]
async fn another_members_reservation_is_not_reached() {
    let (library, copy) = with_copy().await;
    let reservation = done(library.lending.reserve(JANE, copy).await);

    done(library.lending.lend(JOHN, copy, Some(reservation)).await);
    library.relay().await.unwrap();

    assert_eq!(
        library.reservation_fulfilled(reservation).await,
        Some(false)
    );
}

#[tokio::test]
async fn a_refusal_comes_back_in_ok_and_rolls_back_the_copy_and_its_delivery() {
    let library = Library::open(&[]);
    let title = created(library.catalogue.register_title("Dune").await);
    let copies = [
        created(library.catalogue.acquire_copy(title).await),
        created(library.catalogue.acquire_copy(title).await),
        created(library.catalogue.acquire_copy(title).await),
    ];
    library.relay().await.unwrap();
    done(library.lending.lend(JANE, copies[0], None).await);
    done(library.lending.lend(JANE, copies[1], None).await);
    library.relay().await.unwrap();
    let pending = library.lending_deliveries_pending().await;

    let refused = library.lending.lend(JANE, copies[2], None).await;

    assert_eq!(
        refused,
        LendingResponse::Refused {
            reason: "loan-limit-reached"
        }
    );
    assert_eq!(
        library.on_shelf(copies[2]).await,
        Some(true),
        "the loan was rolled back"
    );
    assert_eq!(library.loans_of(copies[2]).await, 0);
    assert_eq!(
        library.loans_recorded().await,
        2,
        "the ledger entry was rolled back"
    );
    assert_eq!(
        library.lending_deliveries_pending().await,
        pending,
        "the outbox entry was rolled back"
    );
}

#[tokio::test]
async fn a_suspended_member_is_refused_by_the_terms_of_lending() {
    let (library, copy) = with_copy().await;
    assert_eq!(
        library.lending.lend(SUSPENDED, copy, None).await,
        LendingResponse::Refused {
            reason: "member-suspended"
        }
    );
    assert_eq!(library.on_shelf(copy).await, Some(true));
    assert_eq!(
        library.lending.reserve(JANE, 777).await,
        LendingResponse::Refused {
            reason: "unknown-copy"
        }
    );
}
