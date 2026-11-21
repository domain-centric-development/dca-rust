//! The context map: the `upstreams`, `external-upstreams` and `partnerships` a context declares in
//! the `[package.metadata.dca]` of its declaring crate, checked against each other, against the
//! Cargo dependencies and against the paths the source names.

use std::collections::{BTreeMap, BTreeSet};

use super::at_line;
use super::graph::crate_location;
use crate::Violation;
use crate::model::{Crate, Layer, Workspace};

const CHANNELS: &[&str] = &["api", "events"];
const TRANSLATIONS: &[&str] = &["acl", "conformist"];
const STATUSES: &[&str] = &["implemented", "planned"];
const INTERACTIONS: &[&str] = &["outbound", "inbound"];

fn at_crate(ws: &Workspace, rule: &'static str, c: &Crate, message: String) -> Violation {
    Violation {
        rule,
        location: crate_location(ws, c),
        message,
    }
}

/// Each context with its declaring crate.
fn declaring(ws: &Workspace) -> Vec<(String, usize)> {
    ws.contexts()
        .into_iter()
        .filter_map(|ctx| ws.declaring_crate(&ctx).map(|k| (ctx, k)))
        .collect()
}

/// The crates of a context.
fn crates_of<'a>(ws: &'a Workspace, ctx: &'a str) -> impl Iterator<Item = usize> + 'a {
    (0..ws.crates.len()).filter(move |k| ws.crates[*k].context.as_deref() == Some(ctx))
}

/// Every path a crate's modules name into another context's api or events crate: (module, line, text,
/// target context, channel).
fn channel_uses(ws: &Workspace, k: usize) -> Vec<(usize, usize, String, String, &'static str)> {
    let own = ws.crates[k].context.clone();
    let mut out = vec![];
    for m in ws.project_modules().filter(|m| ws.modules[*m].krate == k) {
        let mut seen = BTreeSet::new();
        for (t, line, text) in ws.module_targets(m) {
            let Some(tm) = ws.target_module(&t) else {
                continue;
            };
            if let Some((ctx, channel)) = ws.channel_of(tm)
                && Some(&ctx) != own.as_ref()
                && seen.insert(text.clone())
            {
                out.push((m, line, text, ctx, channel));
            }
        }
    }
    out
}

/// Whether a crate has a normal dependency on crate `to`.
fn depends_on(ws: &Workspace, k: usize, to: usize) -> bool {
    ws.crates[k]
        .deps
        .iter()
        .any(|d| d.normal && ws.dep_crate(d) == Some(to))
}

/// DCA-MAP-001: context-map declarations sit only on the crate that declares the context.
pub(super) fn map_001(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for (k, c) in ws.crates.iter().enumerate() {
        if c.map.is_empty() {
            continue;
        }
        let declares = c
            .context
            .as_deref()
            .is_some_and(|ctx| ws.declaring_crate(ctx) == Some(k));
        if !declares {
            out.push(at_crate(ws, "DCA-MAP-001", c, format!(
                "declares upstreams, external-upstreams or partnerships, but `{}` is not the crate that declares a context - put them on the context's domain crate",
                c.package
            )));
        }
    }
    out
}

/// DCA-MAP-002: an external upstream has a name that is no context, a known interaction and
/// translation, and occurs once per (name, interaction).
pub(super) fn map_002(ws: &Workspace) -> Vec<Violation> {
    let contexts = ws.contexts();
    let mut out = vec![];
    for (ctx, k) in declaring(ws) {
        let c = &ws.crates[k];
        let mut seen = BTreeSet::new();
        for e in &c.map.external_upstreams {
            let mut say = |m: String| out.push(at_crate(ws, "DCA-MAP-002", c, m));
            if e.name.trim().is_empty() {
                say(format!(
                    "context `{ctx}` declares an external upstream without a name"
                ));
            } else if contexts.contains(&e.name) {
                say(format!(
                    "external upstream `{}` of `{ctx}` is a bounded context of the workspace; declare it under upstreams",
                    e.name
                ));
            }
            if !INTERACTIONS.contains(&e.interaction.as_str()) {
                say(format!(
                    "external upstream `{}` of `{ctx}` has the interaction `{}`; use outbound or inbound",
                    e.name, e.interaction
                ));
            }
            if !TRANSLATIONS.contains(&e.translation.as_str()) {
                say(format!(
                    "external upstream `{}` of `{ctx}` has the translation `{}`; use acl or conformist",
                    e.name, e.translation
                ));
            }
            if !STATUSES.contains(&e.status.as_str()) {
                say(format!(
                    "external upstream `{}` of `{ctx}` has the status `{}`; use implemented or planned",
                    e.name, e.status
                ));
            }
            if !seen.insert((e.name.clone(), e.interaction.clone())) {
                say(format!(
                    "context `{ctx}` declares the external upstream `{}` ({}) twice",
                    e.name, e.interaction
                ));
            }
        }
    }
    out
}

/// DCA-MAP-004: an upstream names an existing context, not the declaring one, with a known
/// translation and status.
pub(super) fn map_004(ws: &Workspace) -> Vec<Violation> {
    let contexts = ws.contexts();
    let mut out = vec![];
    for (ctx, k) in declaring(ws) {
        let c = &ws.crates[k];
        for u in &c.map.upstreams {
            let mut say = |m: String| out.push(at_crate(ws, "DCA-MAP-004", c, m));
            if u.context == ctx {
                say(format!("context `{ctx}` declares itself as its upstream"));
            } else if !contexts.contains(&u.context) {
                say(format!(
                    "context `{ctx}` declares the upstream `{}`, which no crate declares as its context",
                    u.context
                ));
            }
            if !TRANSLATIONS.contains(&u.translation.as_str()) {
                say(format!(
                    "upstream `{}` of `{ctx}` has the translation `{}`; use acl or conformist",
                    u.context, u.translation
                ));
            }
            if !STATUSES.contains(&u.status.as_str()) {
                say(format!(
                    "upstream `{}` of `{ctx}` has the status `{}`; use implemented or planned",
                    u.context, u.status
                ));
            }
        }
    }
    out
}

/// DCA-MAP-005: an upstream names at least one known channel, and (context, channel) occurs once.
pub(super) fn map_005(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for (ctx, k) in declaring(ws) {
        let c = &ws.crates[k];
        let mut seen = BTreeSet::new();
        for u in &c.map.upstreams {
            let mut say = |m: String| out.push(at_crate(ws, "DCA-MAP-005", c, m));
            if u.via.is_empty() {
                say(format!(
                    "upstream `{}` of `{ctx}` names no channel in via; name api, events or both",
                    u.context
                ));
            }
            for ch in &u.via {
                if !CHANNELS.contains(&ch.as_str()) {
                    say(format!(
                        "upstream `{}` of `{ctx}` names the channel `{ch}`; use api or events",
                        u.context
                    ));
                } else if !seen.insert((u.context.clone(), ch.clone())) {
                    say(format!(
                        "context `{ctx}` declares the upstream `{}` via {ch} twice",
                        u.context
                    ));
                }
            }
        }
    }
    out
}

/// DCA-MAP-006: the contexts a context declares as upstreams are the contexts whose api or events crate its
/// crates depend on.
pub(super) fn map_006(ws: &Workspace) -> Vec<Violation> {
    let contexts = ws.contexts();
    let mut out = vec![];
    for (ctx, k) in declaring(ws) {
        let declared: BTreeSet<String> = ws.crates[k]
            .map
            .upstreams
            .iter()
            .map(|u| u.context.clone())
            .filter(|t| *t != ctx && contexts.contains(t))
            .collect();
        let mut depended: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for from in crates_of(ws, &ctx) {
            for dep in ws.crates[from].deps.iter().filter(|d| d.normal) {
                if let Some(to) = ws.dep_crate(dep)
                    && ws.crates[to].layer.is_some_and(Layer::is_published)
                    && let Some(t) = ws.crates[to].context.clone()
                    && t != ctx
                {
                    depended.entry(t).or_default().push(from);
                }
            }
        }
        for t in declared.iter().filter(|t| !depended.contains_key(*t)) {
            out.push(at_crate(ws, "DCA-MAP-006", &ws.crates[k], format!(
                "context `{ctx}` declares the upstream `{t}`, but none of its crates depends on the api or events crate of `{t}`"
            )));
        }
        for (t, froms) in depended.iter().filter(|(t, _)| !declared.contains(*t)) {
            for from in froms {
                out.push(at_crate(ws, "DCA-MAP-006", &ws.crates[*from], format!(
                    "depends on a published crate of `{t}`, which context `{ctx}` does not declare as an upstream"
                )));
            }
        }
    }
    out
}

/// DCA-MAP-007: an implemented upstream is backed, for each channel, by a path the context's
/// source names into that channel.
pub(super) fn map_007(ws: &Workspace) -> Vec<Violation> {
    let contexts = ws.contexts();
    let mut out = vec![];
    for (ctx, k) in declaring(ws) {
        let used: BTreeSet<(String, &str)> = crates_of(ws, &ctx)
            .flat_map(|c| channel_uses(ws, c))
            .map(|(_, _, _, t, ch)| (t, ch))
            .collect();
        for u in ws.crates[k]
            .map
            .upstreams
            .iter()
            .filter(|u| u.implemented())
        {
            if u.context == ctx || !contexts.contains(&u.context) {
                continue;
            }
            for ch in u.via.iter().filter(|ch| CHANNELS.contains(&ch.as_str())) {
                if !used.contains(&(u.context.clone(), ch.as_str())) {
                    out.push(at_crate(ws, "DCA-MAP-007", &ws.crates[k], format!(
                        "context `{ctx}` declares the implemented upstream `{}` via {ch}, but no source file of `{ctx}` names its {ch} channel - remove the edge or mark it planned",
                        u.context
                    )));
                }
            }
        }
    }
    out
}

/// DCA-MAP-008: an anti-corruption layer keeps the upstream's channels inside the adapters crate,
/// the api channel in the outgoing module, the events channel in the incoming module; for an
/// implemented declaration, a module there names both the channel and its own domain or
/// application, alone or together with a module of that side it names or that names it.
pub(super) fn map_008(ws: &Workspace) -> Vec<Violation> {
    let contexts = ws.contexts();
    let mut out = vec![];
    for (ctx, k) in declaring(ws) {
        for u in ws.crates[k]
            .map
            .upstreams
            .iter()
            .filter(|u| u.translation == "acl")
        {
            if u.context == ctx || !contexts.contains(&u.context) {
                continue;
            }
            for api in ws.published_crates(&u.context) {
                for from in crates_of(ws, &ctx) {
                    if ws.crates[from].layer != Some(Layer::Adapters) && depends_on(ws, from, api) {
                        out.push(at_crate(ws, "DCA-MAP-008", &ws.crates[from], format!(
                            "depends on `{}`, but `{ctx}` translates `{}` in an anti-corruption layer - only the adapters crate depends on it",
                            ws.crates[api].package, u.context
                        )));
                    }
                }
            }
            if u.implemented() {
                let published: Vec<&str> = ws
                    .published_crates(&u.context)
                    .into_iter()
                    .filter_map(|c| ws.crates[c].layer.map(Layer::name))
                    .collect();
                for ch in u.via.iter().filter(|ch| {
                    CHANNELS.contains(&ch.as_str()) && published.contains(&ch.as_str())
                }) {
                    let side = if ch == "api" { "outgoing" } else { "incoming" };
                    if !translates(ws, &ctx, &u.context, ch, side) {
                        out.push(at_crate(ws, "DCA-MAP-008", &ws.crates[k], format!(
                            "context `{ctx}` translates the {ch} channel of `{}` in an anti-corruption layer, but no module of the {side} side of its adapters crate names both that channel and its own domain or application, alone or with a module it names - the translation site is missing",
                            u.context
                        )));
                    }
                }
            }
            for from in crates_of(ws, &ctx).filter(|c| ws.crates[*c].layer == Some(Layer::Adapters))
            {
                for (m, line, text, t, ch) in channel_uses(ws, from) {
                    if t != u.context || !u.via.iter().any(|v| v == ch) {
                        continue;
                    }
                    let side = if ch == "api" { "outgoing" } else { "incoming" };
                    if !ws.module_in_side(m, side) {
                        out.push(at_line(ws, "DCA-MAP-008", m, "", line, format!(
                            "names `{text}` of the {ch} channel of `{t}` outside the {side} module - the anti-corruption layer translates it there"
                        )));
                    }
                }
            }
        }
    }
    out
}

/// Whether the `side` of an adapters crate of `ctx` holds the translation site for the `channel`
/// of `upstream`: a module there names both that channel and a crate of `ctx` with the layer domain
/// or application, or two modules of the side share the work - one names the channel, the other
/// the own domain or application, and one of them names the other (a listener and its translator).
fn translates(ws: &Workspace, ctx: &str, upstream: &str, channel: &str, side: &str) -> bool {
    struct Site {
        channel: bool,
        own: bool,
        names: BTreeSet<usize>,
    }
    let sites: BTreeMap<usize, Site> = crates_of(ws, ctx)
        .filter(|c| ws.crates[*c].layer == Some(Layer::Adapters))
        .flat_map(|k| {
            ws.project_modules()
                .filter(move |m| ws.modules[*m].krate == k && ws.module_in_side(*m, side))
        })
        .map(|m| {
            let modules: BTreeSet<usize> = ws
                .module_targets(m)
                .iter()
                .filter_map(|(t, _, _)| ws.target_module(t))
                .collect();
            let channel = modules.iter().any(|tm| {
                ws.channel_of(*tm)
                    .is_some_and(|(c, ch)| c == upstream && ch == channel)
            });
            let own = modules.iter().any(|tm| {
                let c = &ws.crates[ws.modules[*tm].krate];
                c.context.as_deref() == Some(ctx)
                    && matches!(c.layer, Some(Layer::Domain | Layer::Application))
            });
            let names = modules.into_iter().filter(|tm| *tm != m).collect();
            (
                m,
                Site {
                    channel,
                    own,
                    names,
                },
            )
        })
        .collect();
    sites.iter().any(|(m, a)| {
        a.channel
            && (a.own
                || sites
                    .iter()
                    .any(|(n, b)| b.own && (a.names.contains(n) || b.names.contains(m))))
    })
}

/// DCA-MAP-009: a conformist keeps the upstream's api and events crates out of its domain crate.
pub(super) fn map_009(ws: &Workspace) -> Vec<Violation> {
    let contexts = ws.contexts();
    let mut out = vec![];
    for (ctx, k) in declaring(ws) {
        for u in ws.crates[k]
            .map
            .upstreams
            .iter()
            .filter(|u| u.translation == "conformist")
        {
            if u.context == ctx || !contexts.contains(&u.context) {
                continue;
            }
            for api in ws.published_crates(&u.context) {
                for from in
                    crates_of(ws, &ctx).filter(|c| ws.crates[*c].layer == Some(Layer::Domain))
                {
                    if depends_on(ws, from, api) {
                        out.push(at_crate(ws, "DCA-MAP-009", &ws.crates[from], format!(
                        "the domain depends on `{}`; `{ctx}` conforms to `{}` in its application and adapters, never in its domain",
                        ws.crates[api].package, u.context
                    )));
                    }
                }
            }
        }
    }
    out
}

/// Whether a dependency names one of the contract crates.
fn is_contract(dep: &crate::model::Dep, contracts: &[String]) -> bool {
    contracts
        .iter()
        .any(|c| *c == dep.package || c.replace('-', "_") == dep.ident)
}

/// DCA-MAP-010: an external system's contract crates stay where its translation puts them.
pub(super) fn map_010(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for (ctx, k) in declaring(ws) {
        for e in ws.crates[k]
            .map
            .external_upstreams
            .iter()
            .filter(|e| !e.contract_crates.is_empty())
        {
            let idents: BTreeSet<String> = e
                .contract_crates
                .iter()
                .map(|c| c.replace('-', "_"))
                .collect();
            for from in crates_of(ws, &ctx) {
                let c = &ws.crates[from];
                let deps: Vec<&String> = c
                    .deps
                    .iter()
                    .filter(|d| d.normal && is_contract(d, &e.contract_crates))
                    .map(|d| &d.package)
                    .collect();
                // The names the source uses for the contract crates: the package names and the
                // alias of a renamed dependency (`feed = { package = "vendor-feed" }`).
                let mut idents = idents.clone();
                idents.extend(
                    c.deps
                        .iter()
                        .filter(|d| is_contract(d, &e.contract_crates))
                        .map(|d| d.ident.clone()),
                );
                let refused = match e.translation.as_str() {
                    "acl" => c.layer != Some(Layer::Adapters),
                    _ => c.layer == Some(Layer::Domain),
                };
                if refused {
                    for d in deps {
                        out.push(at_crate(ws, "DCA-MAP-010", c, format!(
                            "depends on `{d}`, a contract crate of the external upstream `{}` that `{ctx}` translates as {}",
                            e.name, e.translation
                        )));
                    }
                    continue;
                }
                if e.translation != "acl" || c.layer != Some(Layer::Adapters) {
                    continue;
                }
                let side = if e.interaction == "inbound" {
                    "incoming"
                } else {
                    "outgoing"
                };
                for m in ws
                    .project_modules()
                    .filter(|m| ws.modules[*m].krate == from)
                {
                    if ws.module_in_side(m, side) {
                        continue;
                    }
                    let mut seen = BTreeSet::new();
                    for (t, line, text) in ws.module_targets(m) {
                        let contract = match &t {
                            crate::Target::External { krate, .. } => idents.contains(krate),
                            other => ws.target_module(other).is_some_and(|tm| {
                                is_contract_crate(
                                    &ws.crates[ws.modules[tm].krate],
                                    &e.contract_crates,
                                )
                            }),
                        };
                        if contract && seen.insert(text.clone()) {
                            out.push(at_line(ws, "DCA-MAP-010", m, "", line, format!(
                                "names `{text}` of the external upstream `{}` outside the {side} module - its {} anti-corruption layer translates it there",
                                e.name, e.interaction
                            )));
                        }
                    }
                }
            }
        }
    }
    out
}

fn is_contract_crate(c: &Crate, contracts: &[String]) -> bool {
    contracts
        .iter()
        .any(|x| *x == c.package || x.replace('-', "_") == c.ident)
}

/// DCA-MAP-011: every path into another context's channel, and every dependency on its api or events crate,
/// is declared as an upstream.
pub(super) fn map_011(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for ctx in ws.contexts() {
        let decls: Vec<crate::model::UpstreamDecl> = ws
            .declaring_crate(&ctx)
            .map(|k| ws.crates[k].map.upstreams.clone())
            .unwrap_or_default();
        let declared = |t: &str, ch: &str| {
            decls
                .iter()
                .any(|u| u.context == t && u.via.iter().any(|v| v == ch))
        };
        for from in crates_of(ws, &ctx) {
            let c = &ws.crates[from];
            for dep in c.deps.iter().filter(|d| d.normal) {
                if let Some(to) = ws.dep_crate(dep)
                    && ws.crates[to].layer.is_some_and(Layer::is_published)
                    && let Some(t) = ws.crates[to].context.as_deref()
                    && t != ctx
                    && !decls.iter().any(|u| u.context == t)
                {
                    out.push(at_crate(ws, "DCA-MAP-011", c, format!(
                        "depends on a published crate of `{t}`, which context `{ctx}` does not declare as an upstream"
                    )));
                }
            }
            for (m, line, text, t, ch) in channel_uses(ws, from) {
                if !declared(&t, ch) {
                    out.push(at_line(ws, "DCA-MAP-011", m, "", line, format!(
                        "names `{text}` of the {ch} channel of `{t}`, but `{ctx}` declares no upstream `{t}` via {ch}"
                    )));
                }
            }
        }
    }
    out
}

/// DCA-MAP-012: a partnership names another existing context that declares it back.
pub(super) fn map_012(ws: &Workspace) -> Vec<Violation> {
    let contexts = ws.contexts();
    let mut out = vec![];
    for (ctx, k) in declaring(ws) {
        let c = &ws.crates[k];
        for p in c.map.partnerships.iter().map(|p| &p.context) {
            let message = if *p == ctx {
                format!("context `{ctx}` declares a partnership with itself")
            } else if !contexts.contains(p) {
                format!(
                    "context `{ctx}` declares a partnership with `{p}`, which no crate declares as its context"
                )
            } else if !ws.declaring_crate(p).is_some_and(|o| {
                ws.crates[o]
                    .map
                    .partnerships
                    .iter()
                    .any(|q| q.context == ctx)
            }) {
                format!(
                    "context `{ctx}` declares a partnership with `{p}`, which does not declare it back"
                )
            } else {
                continue;
            };
            out.push(at_crate(ws, "DCA-MAP-012", c, message));
        }
    }
    out
}

/// DCA-MAP-013 (informational): every declared edge.
pub(super) fn map_013(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for (ctx, k) in declaring(ws) {
        let c = &ws.crates[k];
        for u in &c.map.upstreams {
            out.push(at_crate(
                ws,
                "DCA-MAP-013",
                c,
                format!(
                    "{ctx} -> {} via {} ({}){}",
                    u.context,
                    u.via.join(", "),
                    u.translation,
                    because(&u.rationale)
                ),
            ));
        }
        for e in &c.map.external_upstreams {
            out.push(at_crate(
                ws,
                "DCA-MAP-013",
                c,
                format!(
                    "{ctx} -> external `{}` {} ({}){}{}{}",
                    e.name,
                    e.interaction,
                    e.translation,
                    labelled("protocol", &e.protocol),
                    labelled("exchanges", &e.exchanges),
                    because(&e.rationale)
                ),
            ));
        }
        for p in &c.map.partnerships {
            out.push(at_crate(
                ws,
                "DCA-MAP-013",
                c,
                format!(
                    "{ctx} <-> {} (partnership){}",
                    p.context,
                    because(&p.rationale)
                ),
            ));
        }
    }
    out
}

/// `: <rationale>` when one is declared.
fn because(rationale: &str) -> String {
    if rationale.is_empty() {
        String::new()
    } else {
        format!(": {rationale}")
    }
}

/// `, <label> <value>` when the value is declared.
fn labelled(label: &str, value: &str) -> String {
    if value.is_empty() {
        String::new()
    } else {
        format!(", {label} {value}")
    }
}

/// DCA-MAP-014 (experimental): every dependency of a context's crate on another context's crate is
/// named by its source. A dependency on a published crate of a planned upstream is not selected.
pub(super) fn map_014(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for (k, c) in ws.crates.iter().enumerate() {
        let Some(ctx) = c.context.as_deref() else {
            continue;
        };
        let planned: BTreeSet<String> = ws
            .declaring_crate(ctx)
            .map(|d| {
                ws.crates[d]
                    .map
                    .upstreams
                    .iter()
                    .filter(|u| !u.implemented())
                    .map(|u| u.context.clone())
                    .collect()
            })
            .unwrap_or_default();
        let named: BTreeSet<usize> = ws
            .project_modules()
            .filter(|m| ws.modules[*m].krate == k)
            .flat_map(|m| ws.module_targets(m))
            .filter_map(|(t, _, _)| ws.target_module(&t).map(|tm| ws.modules[tm].krate))
            .collect();
        for dep in c.deps.iter().filter(|d| d.normal) {
            let Some(to) = ws.dep_crate(dep) else {
                continue;
            };
            let other = &ws.crates[to];
            let Some(octx) = other.context.as_deref() else {
                continue;
            };
            if octx == ctx
                || (other.layer.is_some_and(Layer::is_published) && planned.contains(octx))
            {
                continue;
            }
            if !named.contains(&to) {
                out.push(at_crate(ws, "DCA-MAP-014", c, format!(
                    "depends on `{}` of context `{octx}`, but no source file of the crate names it - remove the dependency",
                    other.package
                )));
            }
        }
    }
    out
}
