//! The workspace as the rules see it: crates with their layer and dependencies, modules with their
//! items, and an index that resolves paths and knows each type's building-block roles.

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use cargo_metadata::{DependencyKind, MetadataCommand};
use syn::spanned::Spanned;
use syn::visit::Visit;

/// The role a crate plays, from `layer` in its `[package.metadata.dca]`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum Layer {
    Domain,
    Application,
    Adapters,
    Api,
    Infrastructure,
    SharedKernel,
    Bootstrap,
}

impl Layer {
    fn parse(s: &str) -> Option<Layer> {
        Some(match s {
            "domain" => Layer::Domain,
            "application" => Layer::Application,
            "adapters" => Layer::Adapters,
            "api" => Layer::Api,
            "infrastructure" => Layer::Infrastructure,
            "shared-kernel" => Layer::SharedKernel,
            "bootstrap" => Layer::Bootstrap,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Layer::Domain => "domain",
            Layer::Application => "application",
            Layer::Adapters => "adapters",
            Layer::Api => "api",
            Layer::Infrastructure => "infrastructure",
            Layer::SharedKernel => "shared-kernel",
            Layer::Bootstrap => "bootstrap",
        }
    }
}

/// One dependency a crate declares.
#[derive(Debug, Clone)]
pub struct Dep {
    /// The package name.
    pub package: String,
    /// The name the source uses for it (`rename`, or the package name with `-` as `_`).
    pub ident: String,
    /// A normal dependency: not dev, not build.
    pub normal: bool,
}

/// A workspace member.
#[derive(Debug)]
pub struct Crate {
    pub package: String,
    pub ident: String,
    pub context: Option<String>,
    pub layer: Option<Layer>,
    pub deps: Vec<Dep>,
    pub manifest_dir: PathBuf,
    pub metadata: serde_json::Value,
    /// The crate's root module, if it has a library or binary target.
    pub root: Option<usize>,
}

/// A module: a file or an inline `mod { }`.
#[derive(Debug)]
pub struct Module {
    pub krate: usize,
    /// Path below the crate root; empty for the root.
    pub path: Vec<String>,
    pub file: PathBuf,
    /// The module's items, child modules taken out.
    pub items: Vec<syn::Item>,
    pub children: Vec<(String, usize)>,
    pub parent: Option<usize>,
    /// The line of the `mod` item that declares it; 1 for a crate root.
    pub line: usize,
}

/// Configuration from `[workspace.metadata.dca]`.
#[derive(Debug, Clone)]
pub struct Config {
    /// "all", or the experimental rule ids switched on.
    pub experimental: Experimental,
    pub disabled: BTreeSet<String>,
    /// Crates a domain crate may depend on besides the building blocks and the shared kernel.
    pub domain_allowed_crates: BTreeSet<String>,
    pub building_blocks: String,
    /// Crates whose derives and attributes a domain type must not carry.
    pub framework_crates: BTreeSet<String>,
    /// Type names that count as an event timestamp.
    pub timestamp_types: BTreeSet<String>,
    /// Submodule of `incoming` that holds event consumers.
    pub event_consumer_module: String,
}

#[derive(Debug, Clone)]
pub enum Experimental {
    All,
    Only(BTreeSet<String>),
}

impl Config {
    fn from_json(v: &serde_json::Value) -> Config {
        let list = |key: &str, default: &[&str]| -> BTreeSet<String> {
            match v.get(key).and_then(|x| x.as_array()) {
                Some(a) => a
                    .iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect(),
                None => default.iter().map(|s| s.to_string()).collect(),
            }
        };
        let experimental = match v.get("experimental") {
            Some(serde_json::Value::String(s)) if s == "all" => Experimental::All,
            _ => Experimental::Only(list("experimental", &[])),
        };
        Config {
            experimental,
            disabled: list("disabled", &[]),
            domain_allowed_crates: list("domain-allowed-crates", &[]),
            building_blocks: v
                .get("building-blocks")
                .and_then(|x| x.as_str())
                .unwrap_or("dca-building-blocks")
                .to_string(),
            framework_crates: list(
                "framework-crates",
                &[
                    "sqlx",
                    "diesel",
                    "sea_orm",
                    "axum",
                    "actix_web",
                    "rocket",
                    "warp",
                    "poem",
                    "tonic",
                    "shaku",
                ],
            ),
            timestamp_types: list(
                "timestamp-types",
                &["Timestamp", "SystemTime", "DateTime", "OffsetDateTime"],
            ),
            event_consumer_module: v
                .get("event-consumer-module")
                .and_then(|x| x.as_str())
                .unwrap_or("event")
                .to_string(),
        }
    }
}

/// What a path resolves to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Module(usize),
    Decl(usize),
    /// A crate outside the workspace (or std), with the segments below its root.
    External {
        krate: String,
        path: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclKind {
    Struct,
    Enum,
    Trait,
    Fn,
    TypeAlias,
    Const,
}

/// A named item a path can point at.
#[derive(Debug)]
pub struct Decl {
    pub module: usize,
    pub name: String,
    pub kind: DeclKind,
    pub item: usize,
    pub line: usize,
}

#[derive(Debug, Clone)]
enum ScopeEntry {
    Decl(usize),
    Module(usize),
    Use(Vec<String>),
}

/// An `impl` block with what its trait and self type resolve to.
#[derive(Debug)]
pub struct ImplInfo {
    pub module: usize,
    pub item: usize,
    pub self_decl: Option<usize>,
    pub trait_target: Option<Target>,
}

/// The building-block roles and the roles each implies.
pub const ROLES: &[(&str, &[&str])] = &[
    ("Value", &[]),
    ("Id", &["Value"]),
    ("Entity", &[]),
    ("AggregateRoot", &["Entity"]),
    ("DomainEvent", &[]),
    ("IntegrationEvent", &[]),
    ("DomainService", &[]),
    ("Factory", &[]),
    ("Specification", &[]),
    ("DomainFailure", &[]),
    ("UseCaseFailure", &[]),
    ("OpenHostService", &[]),
    ("InputPort", &[]),
    ("UseCase", &["InputPort"]),
    ("OutputPort", &[]),
    ("Repository", &["OutputPort"]),
    ("Store", &["OutputPort"]),
    ("DomainEventPublisher", &["OutputPort"]),
    ("IntegrationEventPublisher", &["OutputPort"]),
    ("TransactionBoundary", &[]),
];

fn role_closure(name: &str) -> Vec<String> {
    let mut out = vec![name.to_string()];
    if let Some((_, parents)) = ROLES.iter().find(|(n, _)| *n == name) {
        for p in *parents {
            out.extend(role_closure(p));
        }
    }
    out
}

/// The loaded workspace.
#[derive(Debug)]
pub struct Workspace {
    pub root: PathBuf,
    pub crates: Vec<Crate>,
    pub modules: Vec<Module>,
    pub decls: Vec<Decl>,
    pub impls: Vec<ImplInfo>,
    pub config: Config,
    /// Files that could not be read or parsed.
    pub problems: Vec<String>,
    scope: Vec<HashMap<String, Vec<ScopeEntry>>>,
    globs: Vec<Vec<Vec<String>>>,
    /// Roles of each decl: carried (a trait) or taken (a type implementing one).
    pub roles: Vec<BTreeSet<String>>,
    /// Every trait each type decl implements, as resolved.
    pub implements: Vec<Vec<Target>>,
}

impl Workspace {
    /// Loads the workspace whose manifest is at or above `dir`.
    pub fn load(dir: &Path) -> Result<Workspace, String> {
        let manifest = find_workspace_manifest(dir)
            .ok_or_else(|| format!("no Cargo.toml at or above {}", dir.display()))?;
        let md = MetadataCommand::new()
            .manifest_path(&manifest)
            .no_deps()
            .exec()
            .map_err(|e| format!("cargo metadata failed: {e}"))?;
        let config = Config::from_json(
            md.workspace_metadata
                .get("dca")
                .unwrap_or(&serde_json::Value::Null),
        );
        let mut ws = Workspace {
            root: md.workspace_root.clone().into_std_path_buf(),
            crates: vec![],
            modules: vec![],
            decls: vec![],
            impls: vec![],
            config,
            problems: vec![],
            scope: vec![],
            globs: vec![],
            roles: vec![],
            implements: vec![],
        };
        let mut packages: Vec<_> = md.workspace_packages();
        packages.sort_by_key(|a| a.name.to_string());
        for pkg in packages {
            let dca = pkg
                .metadata
                .get("dca")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let target = pkg
                .targets
                .iter()
                .find(|t| t.is_lib())
                .or_else(|| pkg.targets.iter().find(|t| t.is_bin()));
            let ident = target
                .map(|t| t.name.replace('-', "_"))
                .unwrap_or_else(|| pkg.name.to_string().replace('-', "_"));
            let deps = pkg
                .dependencies
                .iter()
                .map(|d| Dep {
                    package: d.name.clone(),
                    ident: d
                        .rename
                        .clone()
                        .unwrap_or_else(|| d.name.clone())
                        .replace('-', "_"),
                    normal: d.kind == DependencyKind::Normal,
                })
                .collect();
            let krate = ws.crates.len();
            ws.crates.push(Crate {
                package: pkg.name.to_string(),
                ident,
                context: dca
                    .get("context")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                layer: dca
                    .get("layer")
                    .and_then(|v| v.as_str())
                    .and_then(Layer::parse),
                deps,
                manifest_dir: pkg
                    .manifest_path
                    .parent()
                    .map(|p| p.to_path_buf().into_std_path_buf())
                    .unwrap_or_default(),
                metadata: dca,
                root: None,
            });
            if let Some(t) = target {
                let src = t.src_path.clone().into_std_path_buf();
                let dir = src.parent().map(Path::to_path_buf).unwrap_or_default();
                match read_file(&src) {
                    Ok(items) => {
                        let root = ws.add_module(krate, vec![], src, items, dir, None);
                        ws.crates[krate].root = Some(root);
                    }
                    Err(e) => ws.problems.push(e),
                }
            }
        }
        ws.index();
        Ok(ws)
    }

    fn add_module(
        &mut self,
        krate: usize,
        path: Vec<String>,
        file: PathBuf,
        items: Vec<syn::Item>,
        child_dir: PathBuf,
        parent: Option<usize>,
    ) -> usize {
        let id = self.modules.len();
        self.modules.push(Module {
            krate,
            path: path.clone(),
            file: file.clone(),
            items: vec![],
            children: vec![],
            parent,
            line: 1,
        });
        let mut own = vec![];
        let mut children = vec![];
        for item in items {
            if is_cfg_test(attrs_of(&item)) {
                continue;
            }
            if let syn::Item::Mod(m) = item {
                let name = m.ident.to_string();
                let mod_line = m.ident.span().start().line;
                let mut child_path = path.clone();
                child_path.push(name.clone());
                let child = match m.content {
                    Some((_, inner)) => Some(self.add_module(
                        krate,
                        child_path,
                        file.clone(),
                        inner,
                        child_dir.join(&name),
                        Some(id),
                    )),
                    None => {
                        let flat = child_dir.join(format!("{name}.rs"));
                        let nested = child_dir.join(&name).join("mod.rs");
                        let chosen = if flat.exists() { flat } else { nested };
                        match read_file(&chosen) {
                            Ok(inner) => Some(self.add_module(
                                krate,
                                child_path,
                                chosen,
                                inner,
                                child_dir.join(&name),
                                Some(id),
                            )),
                            Err(e) => {
                                self.problems.push(e);
                                None
                            }
                        }
                    }
                };
                if let Some(c) = child {
                    self.modules[c].line = mod_line;
                    children.push((name, c));
                }
            } else {
                own.push(item);
            }
        }
        self.modules[id].items = own;
        self.modules[id].children = children;
        id
    }

    fn index(&mut self) {
        let n = self.modules.len();
        self.scope = vec![HashMap::new(); n];
        self.globs = vec![vec![]; n];
        for m in 0..n {
            let children = self.modules[m].children.clone();
            for (name, c) in children {
                self.scope[m]
                    .entry(name)
                    .or_default()
                    .push(ScopeEntry::Module(c));
            }
            let items = std::mem::take(&mut self.modules[m].items);
            for (i, item) in items.iter().enumerate() {
                let (name, kind) = match item {
                    syn::Item::Struct(s) => (s.ident.to_string(), DeclKind::Struct),
                    syn::Item::Enum(e) => (e.ident.to_string(), DeclKind::Enum),
                    syn::Item::Trait(t) => (t.ident.to_string(), DeclKind::Trait),
                    syn::Item::Fn(f) => (f.sig.ident.to_string(), DeclKind::Fn),
                    syn::Item::Type(t) => (t.ident.to_string(), DeclKind::TypeAlias),
                    syn::Item::Const(c) => (c.ident.to_string(), DeclKind::Const),
                    syn::Item::Use(u) => {
                        for (alias, path) in flatten_use(&u.tree, vec![]) {
                            if alias == "*" {
                                self.globs[m].push(path);
                            } else {
                                self.scope[m]
                                    .entry(alias)
                                    .or_default()
                                    .push(ScopeEntry::Use(path));
                            }
                        }
                        continue;
                    }
                    _ => continue,
                };
                let d = self.decls.len();
                self.decls.push(Decl {
                    module: m,
                    name: name.clone(),
                    kind,
                    item: i,
                    line: line_of(item),
                });
                self.scope[m]
                    .entry(name)
                    .or_default()
                    .push(ScopeEntry::Decl(d));
            }
            self.modules[m].items = items;
        }
        self.roles = vec![BTreeSet::new(); self.decls.len()];
        self.implements = vec![vec![]; self.decls.len()];
        for m in 0..n {
            for (i, item) in self.modules[m].items.iter().enumerate() {
                if let syn::Item::Impl(imp) = item {
                    let self_decl = self.type_decl(m, &imp.self_ty, &imp.generics);
                    let trait_target = imp
                        .trait_
                        .as_ref()
                        .and_then(|(_, p, _)| self.resolve_path(m, p));
                    self.impls.push(ImplInfo {
                        module: m,
                        item: i,
                        self_decl,
                        trait_target,
                    });
                }
            }
        }
        for imp in 0..self.impls.len() {
            if let (Some(d), Some(t)) = (
                self.impls[imp].self_decl,
                self.impls[imp].trait_target.clone(),
            ) {
                self.implements[d].push(t);
            }
        }
        // Roles: a trait carries the roles of its supertraits; a type takes the roles of what it implements.
        let mut changed = true;
        while changed {
            changed = false;
            for d in 0..self.decls.len() {
                let mut add: BTreeSet<String> = BTreeSet::new();
                if self.decls[d].kind == DeclKind::Trait {
                    let m = self.decls[d].module;
                    if let syn::Item::Trait(t) = &self.modules[m].items[self.decls[d].item] {
                        for b in &t.supertraits {
                            if let syn::TypeParamBound::Trait(tb) = b
                                && let Some(target) = self.resolve_path(m, &tb.path)
                            {
                                add.extend(self.roles_of_target(&target));
                            }
                        }
                    }
                }
                for t in &self.implements[d] {
                    add.extend(self.roles_of_target(t));
                }
                for r in add {
                    if self.roles[d].insert(r) {
                        changed = true;
                    }
                }
            }
        }
    }

    /// The building-block name a target is, if it is one.
    pub fn building_block(&self, t: &Target) -> Option<String> {
        let bb_ident = self.config.building_blocks.replace('-', "_");
        match t {
            Target::Decl(d) => {
                let k = self.modules[self.decls[*d].module].krate;
                if self.crates[k].package == self.config.building_blocks {
                    Some(self.decls[*d].name.clone())
                } else {
                    None
                }
            }
            Target::External { krate, path } if *krate == bb_ident => path.last().cloned(),
            _ => None,
        }
    }

    fn roles_of_target(&self, t: &Target) -> BTreeSet<String> {
        if let Some(bb) = self.building_block(t) {
            if ROLES.iter().any(|(n, _)| *n == bb) {
                return role_closure(&bb).into_iter().collect();
            }
            return BTreeSet::new();
        }
        match t {
            Target::Decl(d) if self.decls[*d].kind == DeclKind::Trait => self.roles[*d].clone(),
            _ => BTreeSet::new(),
        }
    }

    /// The decl a type names, generics and references looked through to the outer path.
    fn type_decl(&self, m: usize, ty: &syn::Type, generics: &syn::Generics) -> Option<usize> {
        match ty {
            syn::Type::Path(tp) if tp.qself.is_none() => {
                if tp.path.segments.len() == 1
                    && generics
                        .type_params()
                        .any(|p| p.ident == tp.path.segments[0].ident)
                {
                    return None;
                }
                match self.resolve_path(m, &tp.path)? {
                    Target::Decl(d) => Some(d),
                    _ => None,
                }
            }
            syn::Type::Reference(r) => self.type_decl(m, &r.elem, generics),
            syn::Type::Paren(p) => self.type_decl(m, &p.elem, generics),
            _ => None,
        }
    }

    /// Resolves a path written in module `m`.
    pub fn resolve_path(&self, m: usize, path: &syn::Path) -> Option<Target> {
        let segs: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();
        if path.leading_colon.is_some() {
            return self.resolve_extern(m, &segs);
        }
        self.resolve(m, &segs, 0)
    }

    /// Resolves segments written in module `m`.
    pub fn resolve(&self, m: usize, segs: &[String], depth: usize) -> Option<Target> {
        if segs.is_empty() || depth > 24 {
            return None;
        }
        let krate = self.modules[m].krate;
        let (mut cur, rest) = match segs[0].as_str() {
            "crate" => (Target::Module(self.crates[krate].root?), &segs[1..]),
            "self" => (Target::Module(m), &segs[1..]),
            "super" => {
                let mut cur = m;
                let mut i = 0;
                while i < segs.len() && segs[i] == "super" {
                    cur = self.modules[cur].parent?;
                    i += 1;
                }
                (Target::Module(cur), &segs[i..])
            }
            "Self" => return None,
            name => match self.lookup_scope(m, name, depth) {
                Some(t) => (t, &segs[1..]),
                None => match self.resolve_extern(m, segs) {
                    Some(t) => return Some(t),
                    None => (self.lookup_glob(m, name, depth)?, &segs[1..]),
                },
            },
        };
        for seg in rest {
            cur = match cur {
                Target::Module(mm) => self.lookup(mm, seg, depth + 1)?,
                Target::Decl(_) => return Some(cur),
                Target::External { krate, mut path } => {
                    path.push(seg.clone());
                    Target::External { krate, path }
                }
            };
        }
        Some(cur)
    }

    fn resolve_extern(&self, m: usize, segs: &[String]) -> Option<Target> {
        let krate = self.modules[m].krate;
        let first = segs.first()?;
        if let Some(dep) = self.crates[krate].deps.iter().find(|d| &d.ident == first) {
            if let Some(k) = self.crates.iter().position(|c| c.package == dep.package) {
                let root = self.crates[k].root?;
                let mut rest = vec!["crate".to_string()];
                rest.extend(segs[1..].iter().cloned());
                return self.resolve(root, &rest, 1);
            }
            return Some(Target::External {
                krate: first.clone(),
                path: segs[1..].to_vec(),
            });
        }
        if matches!(first.as_str(), "std" | "core" | "alloc") {
            return Some(Target::External {
                krate: first.clone(),
                path: segs[1..].to_vec(),
            });
        }
        None
    }

    fn lookup(&self, m: usize, name: &str, depth: usize) -> Option<Target> {
        self.lookup_scope(m, name, depth)
            .or_else(|| self.lookup_glob(m, name, depth))
    }

    /// A name the module declares or imports by name.
    fn lookup_scope(&self, m: usize, name: &str, depth: usize) -> Option<Target> {
        if depth > 24 {
            return None;
        }
        if let Some(entries) = self.scope[m].get(name) {
            for e in entries {
                let t = match e {
                    ScopeEntry::Decl(d) => Some(Target::Decl(*d)),
                    ScopeEntry::Module(c) => Some(Target::Module(*c)),
                    ScopeEntry::Use(p) => self.resolve(m, p, depth + 1),
                };
                if t.is_some() {
                    return t;
                }
            }
        }
        None
    }

    /// A name a glob import brings in. A glob over a crate outside the workspace cannot be listed,
    /// so it answers for any name; it is asked last.
    fn lookup_glob(&self, m: usize, name: &str, depth: usize) -> Option<Target> {
        if depth > 24 {
            return None;
        }
        for g in &self.globs[m] {
            let mut p = g.clone();
            p.push(name.to_string());
            if let Some(t) = self.resolve(m, &p, depth + 1) {
                return Some(t);
            }
        }
        None
    }

    // --- accessors the rules use ----------------------------------------------------------------

    pub fn crate_of_decl(&self, d: usize) -> &Crate {
        &self.crates[self.modules[self.decls[d].module].krate]
    }

    pub fn layer_of_decl(&self, d: usize) -> Option<Layer> {
        self.crate_of_decl(d).layer
    }

    pub fn module_layer(&self, m: usize) -> Option<Layer> {
        self.crates[self.modules[m].krate].layer
    }

    /// Whether module `m` is the adapter side `side` (incoming or outgoing) or below it.
    pub fn module_in_side(&self, m: usize, side: &str) -> bool {
        self.module_layer(m) == Some(Layer::Adapters)
            && self.modules[m].path.first().map(String::as_str) == Some(side)
    }

    pub fn decl_in_side(&self, d: usize, side: &str) -> bool {
        self.module_in_side(self.decls[d].module, side)
    }

    pub fn item(&self, d: usize) -> &syn::Item {
        let decl = &self.decls[d];
        &self.modules[decl.module].items[decl.item]
    }

    pub fn has_role(&self, d: usize, role: &str) -> bool {
        self.roles[d].contains(role)
    }

    /// Struct and enum decls that carry `role`.
    pub fn types_with_role<'a>(&'a self, role: &'a str) -> impl Iterator<Item = usize> + 'a {
        (0..self.decls.len()).filter(move |d| {
            matches!(self.decls[*d].kind, DeclKind::Struct | DeclKind::Enum)
                && self.has_role(*d, role)
        })
    }

    /// Trait decls that carry `role`, the building blocks' own excluded.
    pub fn traits_with_role<'a>(&'a self, role: &'a str) -> impl Iterator<Item = usize> + 'a {
        (0..self.decls.len()).filter(move |d| {
            self.decls[*d].kind == DeclKind::Trait
                && self.has_role(*d, role)
                && !self.is_building_block_decl(*d)
        })
    }

    pub fn is_building_block_decl(&self, d: usize) -> bool {
        self.crate_of_decl(d).package == self.config.building_blocks
    }

    /// Struct and enum decls outside the building blocks.
    pub fn type_decls(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.decls.len()).filter(|d| {
            matches!(self.decls[*d].kind, DeclKind::Struct | DeclKind::Enum)
                && !self.is_building_block_decl(*d)
        })
    }

    pub fn trait_decls(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.decls.len())
            .filter(|d| self.decls[*d].kind == DeclKind::Trait && !self.is_building_block_decl(*d))
    }

    /// Modules outside the building blocks.
    pub fn project_modules(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.modules.len())
            .filter(|m| self.crates[self.modules[*m].krate].package != self.config.building_blocks)
    }

    /// Fields of a struct or of an enum's variants.
    pub fn fields(&self, d: usize) -> Vec<FieldRef<'_>> {
        let mut out = vec![];
        fn push<'a>(out: &mut Vec<FieldRef<'a>>, fields: &'a syn::Fields, variant: Option<String>) {
            for (i, f) in fields.iter().enumerate() {
                out.push(FieldRef {
                    name: f
                        .ident
                        .as_ref()
                        .map(|x| x.to_string())
                        .unwrap_or_else(|| i.to_string()),
                    variant: variant.clone(),
                    ty: &f.ty,
                    public: matches!(f.vis, syn::Visibility::Public(_)),
                    line: f.span().start().line,
                });
            }
        }
        match self.item(d) {
            syn::Item::Struct(s) => push(&mut out, &s.fields, None),
            syn::Item::Enum(e) => {
                for v in &e.variants {
                    push(&mut out, &v.fields, Some(v.ident.to_string()));
                }
            }
            _ => {}
        }
        out
    }

    pub fn generics_of(&self, d: usize) -> Option<&syn::Generics> {
        match self.item(d) {
            syn::Item::Struct(s) => Some(&s.generics),
            syn::Item::Enum(e) => Some(&e.generics),
            syn::Item::Trait(t) => Some(&t.generics),
            _ => None,
        }
    }

    /// Every method of a type: inherent and from trait impls.
    pub fn methods(&self, d: usize) -> Vec<MethodRef<'_>> {
        let mut out = vec![];
        for imp in &self.impls {
            if imp.self_decl != Some(d) {
                continue;
            }
            let syn::Item::Impl(item) = &self.modules[imp.module].items[imp.item] else {
                continue;
            };
            for ii in &item.items {
                if let syn::ImplItem::Fn(f) = ii {
                    out.push(MethodRef {
                        name: f.sig.ident.to_string(),
                        sig: &f.sig,
                        block: Some(&f.block),
                        public: imp.trait_target.is_some()
                            || matches!(f.vis, syn::Visibility::Public(_)),
                        inherent: imp.trait_target.is_none(),
                        trait_target: imp.trait_target.clone(),
                        impl_generics: &item.generics,
                        module: imp.module,
                        line: f.sig.span().start().line,
                    });
                }
            }
        }
        out
    }

    /// Methods a trait declares.
    pub fn trait_methods(&self, d: usize) -> Vec<MethodRef<'_>> {
        let syn::Item::Trait(t) = self.item(d) else {
            return vec![];
        };
        t.items
            .iter()
            .filter_map(|ti| match ti {
                syn::TraitItem::Fn(f) => Some(MethodRef {
                    name: f.sig.ident.to_string(),
                    sig: &f.sig,
                    block: f.default.as_ref(),
                    public: true,
                    inherent: false,
                    trait_target: None,
                    impl_generics: &t.generics,
                    module: self.decls[d].module,
                    line: f.sig.span().start().line,
                }),
                _ => None,
            })
            .collect()
    }

    /// The targets a type expression names: every path in it, generic arguments and trait-object
    /// bounds included; a generic parameter stands for its bounds.
    pub fn type_targets(
        &self,
        m: usize,
        ty: &syn::Type,
        generics: &[&syn::Generics],
    ) -> Vec<Target> {
        let mut c = PathCollector::default();
        c.visit_type(ty);
        let mut out = vec![];
        for p in c.paths {
            if p.segments.len() == 1 {
                let ident = &p.segments[0].ident;
                let bounds = generic_bounds(generics, ident);
                if let Some(bounds) = bounds {
                    for b in bounds {
                        if let Some(t) = self.resolve_path(m, &b) {
                            out.push(t);
                        }
                    }
                    continue;
                }
            }
            if let Some(t) = self.resolve_path(m, &p) {
                out.push(t);
            }
        }
        out
    }

    /// The decls a type expression names.
    pub fn type_decls_in(
        &self,
        m: usize,
        ty: &syn::Type,
        generics: &[&syn::Generics],
    ) -> Vec<usize> {
        self.type_targets(m, ty, generics)
            .into_iter()
            .filter_map(|t| match t {
                Target::Decl(d) => Some(d),
                _ => None,
            })
            .collect()
    }

    /// Every target module `m` names, with the line: `use` items and paths in its other items.
    pub fn module_targets(&self, m: usize) -> Vec<(Target, usize, String)> {
        let mut out = vec![];
        for item in &self.modules[m].items {
            if let syn::Item::Use(u) = item {
                let line = u.span().start().line;
                for (_, path) in flatten_use(&u.tree, vec![]) {
                    if let Some(t) = self.resolve(m, &path, 0) {
                        out.push((t, line, path.join("::")));
                    }
                }
            } else {
                let mut c = PathCollector::default();
                c.visit_item(item);
                for p in c.paths {
                    if let Some(t) = self.resolve_path(m, &p) {
                        let text = p
                            .segments
                            .iter()
                            .map(|s| s.ident.to_string())
                            .collect::<Vec<_>>()
                            .join("::");
                        out.push((t, p.span().start().line, text));
                    }
                }
            }
        }
        out
    }

    /// The module a target lies in, if it is inside the workspace.
    pub fn target_module(&self, t: &Target) -> Option<usize> {
        match t {
            Target::Module(m) => Some(*m),
            Target::Decl(d) => Some(self.decls[*d].module),
            Target::External { .. } => None,
        }
    }

    /// `crate::path::Name (file:line)` for a decl.
    pub fn where_decl(&self, d: usize) -> String {
        let decl = &self.decls[d];
        self.where_in(decl.module, &decl.name, decl.line)
    }

    pub fn where_in(&self, m: usize, name: &str, line: usize) -> String {
        let module = &self.modules[m];
        let mut path = vec![self.crates[module.krate].ident.clone()];
        path.extend(module.path.iter().cloned());
        if !name.is_empty() {
            path.push(name.to_string());
        }
        let file = module.file.strip_prefix(&self.root).unwrap_or(&module.file);
        format!("{} ({}:{})", path.join("::"), file.display(), line)
    }

    pub fn decl_name(&self, d: usize) -> &str {
        &self.decls[d].name
    }

    pub fn target_name(&self, t: &Target) -> String {
        match t {
            Target::Decl(d) => self.decls[*d].name.clone(),
            Target::Module(m) => {
                let mut p = vec![self.crates[self.modules[*m].krate].ident.clone()];
                p.extend(self.modules[*m].path.iter().cloned());
                p.join("::")
            }
            Target::External { krate, path } => {
                let mut p = vec![krate.clone()];
                p.extend(path.iter().cloned());
                p.join("::")
            }
        }
    }

    /// The workspace crate a dependency names.
    pub fn dep_crate(&self, dep: &Dep) -> Option<usize> {
        self.crates.iter().position(|c| c.package == dep.package)
    }
}

/// A field of a struct or of an enum variant.
pub struct FieldRef<'a> {
    pub name: String,
    pub variant: Option<String>,
    pub ty: &'a syn::Type,
    pub public: bool,
    pub line: usize,
}

/// A method of a type or a trait.
pub struct MethodRef<'a> {
    pub name: String,
    pub sig: &'a syn::Signature,
    pub block: Option<&'a syn::Block>,
    pub public: bool,
    pub inherent: bool,
    pub trait_target: Option<Target>,
    pub impl_generics: &'a syn::Generics,
    pub module: usize,
    pub line: usize,
}

impl MethodRef<'_> {
    pub fn receiver_mut(&self) -> bool {
        self.sig.inputs.iter().any(|a| match a {
            syn::FnArg::Receiver(r) => r.reference.is_some() && r.mutability.is_some(),
            _ => false,
        })
    }

    pub fn has_receiver(&self) -> bool {
        self.sig
            .inputs
            .iter()
            .any(|a| matches!(a, syn::FnArg::Receiver(_)))
    }

    pub fn params(&self) -> Vec<&syn::Type> {
        self.sig
            .inputs
            .iter()
            .filter_map(|a| match a {
                syn::FnArg::Typed(t) => Some(&*t.ty),
                _ => None,
            })
            .collect()
    }

    pub fn returns_self(&self) -> bool {
        match &self.sig.output {
            syn::ReturnType::Type(_, ty) => type_mentions_self(ty),
            _ => false,
        }
    }

    pub fn generics(&self) -> Vec<&syn::Generics> {
        vec![self.impl_generics, &self.sig.generics]
    }
}

fn type_mentions_self(ty: &syn::Type) -> bool {
    let mut c = PathCollector::default();
    c.visit_type(ty);
    c.paths
        .iter()
        .any(|p| p.segments.len() == 1 && p.segments[0].ident == "Self")
}

/// Collects every path in what it visits.
#[derive(Default)]
pub struct PathCollector {
    pub paths: Vec<syn::Path>,
}

impl<'ast> Visit<'ast> for PathCollector {
    fn visit_path(&mut self, p: &'ast syn::Path) {
        self.paths.push(p.clone());
        syn::visit::visit_path(self, p);
    }
}

fn generic_bounds(generics: &[&syn::Generics], ident: &syn::Ident) -> Option<Vec<syn::Path>> {
    let mut found = false;
    let mut out = vec![];
    for g in generics {
        for p in g.type_params() {
            if &p.ident == ident {
                found = true;
                for b in &p.bounds {
                    if let syn::TypeParamBound::Trait(tb) = b {
                        out.push(tb.path.clone());
                    }
                }
            }
        }
        if let Some(w) = &g.where_clause {
            for pred in &w.predicates {
                if let syn::WherePredicate::Type(pt) = pred
                    && let syn::Type::Path(tp) = &pt.bounded_ty
                    && tp.path.is_ident(ident)
                {
                    found = true;
                    for b in &pt.bounds {
                        if let syn::TypeParamBound::Trait(tb) = b {
                            out.push(tb.path.clone());
                        }
                    }
                }
            }
        }
    }
    found.then_some(out)
}

fn flatten_use(tree: &syn::UseTree, prefix: Vec<String>) -> Vec<(String, Vec<String>)> {
    match tree {
        syn::UseTree::Path(p) => {
            let mut pre = prefix;
            pre.push(p.ident.to_string());
            flatten_use(&p.tree, pre)
        }
        syn::UseTree::Name(n) => {
            let name = n.ident.to_string();
            if name == "self" {
                let alias = prefix.last().cloned().unwrap_or_default();
                vec![(alias, prefix)]
            } else {
                let mut p = prefix;
                p.push(name.clone());
                vec![(name, p)]
            }
        }
        syn::UseTree::Rename(r) => {
            let mut p = prefix;
            let name = r.ident.to_string();
            if name != "self" {
                p.push(name);
            }
            vec![(r.rename.to_string(), p)]
        }
        syn::UseTree::Glob(_) => vec![("*".to_string(), prefix)],
        syn::UseTree::Group(g) => g
            .items
            .iter()
            .flat_map(|t| flatten_use(t, prefix.clone()))
            .collect(),
    }
}

fn read_file(path: &Path) -> Result<Vec<syn::Item>, String> {
    let src = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    syn::parse_file(&src)
        .map(|f| f.items)
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn find_workspace_manifest(dir: &Path) -> Option<PathBuf> {
    let mut best = None;
    let mut cur = Some(dir);
    while let Some(d) = cur {
        let m = d.join("Cargo.toml");
        if m.exists() {
            if fs::read_to_string(&m)
                .map(|s| s.contains("[workspace]"))
                .unwrap_or(false)
            {
                return Some(m);
            }
            if best.is_none() {
                best = Some(m);
            }
        }
        cur = d.parent();
    }
    best
}

fn attrs_of(item: &syn::Item) -> &[syn::Attribute] {
    match item {
        syn::Item::Mod(m) => &m.attrs,
        syn::Item::Fn(f) => &f.attrs,
        syn::Item::Impl(i) => &i.attrs,
        syn::Item::Struct(s) => &s.attrs,
        syn::Item::Enum(e) => &e.attrs,
        syn::Item::Use(u) => &u.attrs,
        syn::Item::Trait(t) => &t.attrs,
        _ => &[],
    }
}

fn is_cfg_test(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        a.path().is_ident("cfg") && matches!(a.parse_args::<syn::Ident>(), Ok(i) if i == "test")
    })
}

fn line_of(item: &syn::Item) -> usize {
    match item {
        syn::Item::Struct(s) => s.ident.span().start().line,
        syn::Item::Enum(e) => e.ident.span().start().line,
        syn::Item::Trait(t) => t.ident.span().start().line,
        syn::Item::Fn(f) => f.sig.ident.span().start().line,
        _ => item.span().start().line,
    }
}
