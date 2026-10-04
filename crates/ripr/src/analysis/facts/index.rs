//! Generation-owned facts with independent ordered flat and file membership.
//!
//! Parser/cache `FileFacts` remains the owned wire DTO. Its facts are moved into
//! these arenas once; membership is never inferred from a name, span or path.
use super::model::*;
use serde::ser::{SerializeMap, SerializeStruct};
use std::collections::{BTreeMap, BTreeSet};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut, Index, IndexMut};
use std::path::{Path, PathBuf};

/// Offsets cannot be constructed outside this module or exchanged between
/// function/test arenas. They are never serialized or exposed by a reader.
#[derive(Debug)]
pub(super) struct FactId<T> {
    offset: usize,
    kind: PhantomData<fn() -> T>,
}
impl<T> PartialEq for FactId<T> {
    fn eq(&self, other: &Self) -> bool {
        self.offset == other.offset
    }
}
impl<T> Eq for FactId<T> {}
impl<T> PartialOrd for FactId<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<T> Ord for FactId<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.offset.cmp(&other.offset)
    }
}
impl<T> Copy for FactId<T> {}
impl<T> Clone for FactId<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> FactId<T> {
    fn new(offset: usize) -> Self {
        Self {
            offset,
            kind: PhantomData,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct FactArena<T> {
    records: Vec<T>,
}
impl<T> Default for FactArena<T> {
    fn default() -> Self {
        Self {
            records: Vec::new(),
        }
    }
}
impl<T> FactArena<T> {
    pub(super) fn allocate(&mut self, fact: T) -> FactId<T> {
        let id = FactId::new(self.records.len());
        self.records.push(fact);
        id
    }
}
impl<T> Deref for FactArena<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        &self.records
    }
}
impl<T> DerefMut for FactArena<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        &mut self.records
    }
}
impl<T> Index<FactId<T>> for FactArena<T> {
    type Output = T;
    fn index(&self, id: FactId<T>) -> &T {
        &self.records[id.offset]
    }
}
impl<T> IndexMut<FactId<T>> for FactArena<T> {
    fn index_mut(&mut self, id: FactId<T>) -> &mut T {
        &mut self.records[id.offset]
    }
}

fn validate<'a, T: 'a>(
    arena: &[T],
    ids: impl Iterator<Item = &'a FactId<T>>,
    kind: &str,
) -> Result<(), String> {
    for (position, id) in ids.enumerate() {
        if position.is_multiple_of(1024) {
            crate::analysis::cancellation::checkpoint()?;
        }
        if id.offset >= arena.len() {
            return Err(format!(
                "invalid {kind} fact membership: {} >= {}",
                id.offset,
                arena.len()
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Default)]
pub struct RustIndex {
    pub(super) files: BTreeMap<PathBuf, IndexedFileFacts>,
    pub(super) function_order: Vec<FactId<FunctionFact>>,
    pub(super) test_order: Vec<FactId<TestFact>>,
    pub(super) function_facts: FactArena<FunctionFact>,
    pub(super) test_facts: FactArena<TestFact>,
    function_positions: Vec<usize>,
    test_positions: Vec<usize>,
    membership_revision: u64,
    pub package_names: BTreeSet<String>,
    pub include_parents: BTreeMap<PathBuf, ResolvedIncludeParent>,
    pub include_limitations: Vec<RustIncludeLimitation>,
    pub non_utf8_sources: BTreeSet<PathBuf>,
    pub(crate) include_targets: BTreeSet<PathBuf>,
    pub harness_subjects: Vec<HarnessSubjectFact>,
    pub harness_limitations: Vec<HarnessLimitationFact>,
    pub(crate) workspace_authority: Option<WorkspaceRootAuthority>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct IndexedFileFacts {
    pub(super) data: FileData,
    pub(super) functions: Vec<FactId<FunctionFact>>,
    pub(super) tests: Vec<FactId<TestFact>>,
}

/// Non-fact metadata is owned by the containing file, independently of each
/// fact's own `file` identity. Those identities are intentionally not conflated.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileData {
    pub path: PathBuf,
    pub calls: Vec<CallFact>,
    pub returns: Vec<ReturnFact>,
    pub literals: Vec<LiteralFact>,
    pub probe_shapes: Vec<ProbeShapeFact>,
    pub used_lexical_fallback: bool,
    pub module_declarations: Vec<ModuleDeclarationFact>,
    pub unresolved_property_macros: Vec<UnresolvedPropertyMacroFact>,
    pub role_provenance: SourceRoleProvenance,
    pub source: String,
}

/// A borrowed ordered view; it cannot outlive or retain an index generation.
pub struct FactSlice<'a, T> {
    arena: &'a [T],
    order: Option<&'a [FactId<T>]>,
}

// Equality follows the exposed sequence, not arena layout or membership IDs.
// Independently built/cached generations may store equal facts at different offsets.
impl<T: PartialEq> PartialEq for FactSlice<'_, T> {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}
impl<T: Eq> Eq for FactSlice<'_, T> {}
impl<T: std::fmt::Debug> std::fmt::Debug for FactSlice<'_, T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_list().entries(self.iter()).finish()
    }
}

impl<T> Copy for FactSlice<'_, T> {}
impl<T> Clone for FactSlice<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, T> FactSlice<'a, T> {
    pub fn from_slice(facts: &'a [T]) -> Self {
        Self {
            arena: facts,
            order: None,
        }
    }
    pub fn iter(&self) -> FactIter<'a, T> {
        FactIter {
            inner: match self.order {
                Some(order) => FactIterState::Indexed {
                    arena: self.arena,
                    order: order.iter(),
                },
                None => FactIterState::Dense(self.arena.iter()),
            },
        }
    }
    pub fn len(&self) -> usize {
        self.order.map_or(self.arena.len(), <[_]>::len)
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn get(&self, position: usize) -> Option<&'a T> {
        match self.order {
            Some(order) => order.get(position).map(|&id| &self.arena[id.offset]),
            None => self.arena.get(position),
        }
    }
    pub fn at(&self, position: usize) -> &'a T {
        match self.order {
            Some(order) => &self.arena[order[position].offset],
            None => &self.arena[position],
        }
    }
    pub fn first(&self) -> Option<&'a T> {
        self.get(0)
    }
}
#[derive(Clone)]
pub struct FactIter<'a, T> {
    inner: FactIterState<'a, T>,
}
#[derive(Clone)]
enum FactIterState<'a, T> {
    Dense(std::slice::Iter<'a, T>),
    Indexed {
        arena: &'a [T],
        order: std::slice::Iter<'a, FactId<T>>,
    },
}
impl<'a, T> Iterator for FactIter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.inner {
            FactIterState::Dense(iter) => iter.next(),
            FactIterState::Indexed { arena, order } => order.next().map(|&id| &arena[id.offset]),
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        match &self.inner {
            FactIterState::Dense(iter) => iter.size_hint(),
            FactIterState::Indexed { order, .. } => order.size_hint(),
        }
    }
}
impl<T> DoubleEndedIterator for FactIter<'_, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        match &mut self.inner {
            FactIterState::Dense(iter) => iter.next_back(),
            FactIterState::Indexed { arena, order } => {
                order.next_back().map(|&id| &arena[id.offset])
            }
        }
    }
}
impl<T> ExactSizeIterator for FactIter<'_, T> {}
impl<'a, T> IntoIterator for FactSlice<'a, T> {
    type Item = &'a T;
    type IntoIter = FactIter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<'a, T> IntoIterator for &FactSlice<'a, T> {
    type Item = &'a T;
    type IntoIter = FactIter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T> Index<usize> for FactSlice<'_, T> {
    type Output = T;
    fn index(&self, position: usize) -> &T {
        match self.order {
            Some(order) => &self.arena[order[position].offset],
            None => &self.arena[position],
        }
    }
}
impl<T: serde::Serialize> serde::Serialize for FactSlice<'_, T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.iter())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileFactsView<'a> {
    data: &'a FileData,
    pub functions: FactSlice<'a, FunctionFact>,
    pub tests: FactSlice<'a, TestFact>,
}
impl<'a> FileFactsView<'a> {
    pub fn data(&self) -> &'a FileData {
        self.data
    }
}
impl Deref for FileFactsView<'_> {
    type Target = FileData;
    fn deref(&self) -> &FileData {
        self.data
    }
}
impl serde::Serialize for FileFactsView<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("FileFacts", 11)?;
        state.serialize_field("path", &self.path)?;
        state.serialize_field("functions", &self.functions)?;
        state.serialize_field("tests", &self.tests)?;
        state.serialize_field("calls", &self.calls)?;
        state.serialize_field("returns", &self.returns)?;
        state.serialize_field("literals", &self.literals)?;
        state.serialize_field("probe_shapes", &self.probe_shapes)?;
        state.serialize_field("used_lexical_fallback", &self.used_lexical_fallback)?;
        state.serialize_field("module_declarations", &self.module_declarations)?;
        state.serialize_field(
            "unresolved_property_macros",
            &self.unresolved_property_macros,
        )?;
        state.serialize_field("source", &self.source)?;
        state.end()
    }
}

#[derive(Clone, Copy)]
pub struct IndexFiles<'a> {
    index: &'a RustIndex,
}
impl<'a> IndexFiles<'a> {
    #[cfg(test)]
    pub fn at(&self, path: &Path) -> FileFactsView<'a> {
        self.index.file_view(&self.index.files[path])
    }
    pub fn get_key_value(&self, path: &Path) -> Option<(&'a PathBuf, FileFactsView<'a>)> {
        self.index
            .files
            .get_key_value(path)
            .map(|(key, file)| (key, self.index.file_view(file)))
    }
    pub fn get(&self, path: &Path) -> Option<FileFactsView<'a>> {
        self.index
            .files
            .get(path)
            .map(|file| self.index.file_view(file))
    }
    pub fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = (&'a PathBuf, FileFactsView<'a>)> + ExactSizeIterator + use<'a>
    {
        let index = self.index;
        index
            .files
            .iter()
            .map(move |(path, file)| (path, index.file_view(file)))
    }
    pub fn values(
        &self,
    ) -> impl DoubleEndedIterator<Item = FileFactsView<'a>> + ExactSizeIterator + use<'a> {
        self.iter().map(|(_, file)| file)
    }
    #[cfg(test)]
    pub fn keys(
        &self,
    ) -> impl DoubleEndedIterator<Item = &'a PathBuf> + ExactSizeIterator + use<'a> {
        self.index.files.keys()
    }
    pub fn len(&self) -> usize {
        self.index.files.len()
    }
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.index.files.is_empty()
    }
    #[cfg(test)]
    pub fn contains_key(&self, path: &Path) -> bool {
        self.index.files.contains_key(path)
    }
}
impl PartialEq for IndexFiles<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}
impl Eq for IndexFiles<'_> {}
impl std::fmt::Debug for IndexFiles<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_map().entries(self.iter()).finish()
    }
}
impl serde::Serialize for IndexFiles<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.len()))?;
        for (path, file) in self.iter() {
            map.serialize_entry(path, &file)?;
        }
        map.end()
    }
}

impl RustIndex {
    pub fn functions(&self) -> FactSlice<'_, FunctionFact> {
        FactSlice {
            arena: &self.function_facts,
            order: Some(&self.function_order),
        }
    }
    pub fn tests(&self) -> FactSlice<'_, TestFact> {
        FactSlice {
            arena: &self.test_facts,
            order: Some(&self.test_order),
        }
    }
    pub fn files(&self) -> IndexFiles<'_> {
        IndexFiles { index: self }
    }
    fn file_view<'a>(&'a self, file: &'a IndexedFileFacts) -> FileFactsView<'a> {
        FileFactsView {
            data: &file.data,
            functions: FactSlice {
                arena: &self.function_facts,
                order: Some(&file.functions),
            },
            tests: FactSlice {
                arena: &self.test_facts,
                order: Some(&file.tests),
            },
        }
    }
    /// Insert each supplied occurrence once. Shared membership is explicit;
    /// equal names, coordinates and bodies never collapse two occurrences.
    pub fn insert_file(&mut self, path: PathBuf, facts: FileFacts, flat: bool) {
        self.bump_membership_revision();
        let FileFacts {
            path: fact_path,
            functions,
            tests,
            calls,
            returns,
            literals,
            probe_shapes,
            used_lexical_fallback,
            module_declarations,
            unresolved_property_macros,
            role_provenance,
            source,
        } = facts;
        let functions = functions
            .into_iter()
            .map(|fact| self.function_facts.allocate(fact))
            .collect::<Vec<_>>();
        let tests = tests
            .into_iter()
            .map(|fact| self.test_facts.allocate(fact))
            .collect::<Vec<_>>();
        self.function_positions
            .resize(self.function_facts.len(), usize::MAX);
        self.test_positions
            .resize(self.test_facts.len(), usize::MAX);
        if flat {
            for &id in &functions {
                self.function_positions[id.offset] = self.function_order.len();
                self.function_order.push(id);
            }
            for &id in &tests {
                self.test_positions[id.offset] = self.test_order.len();
                self.test_order.push(id);
            }
        }
        self.files.insert(
            path,
            IndexedFileFacts {
                data: FileData {
                    path: fact_path,
                    calls,
                    returns,
                    literals,
                    probe_shapes,
                    used_lexical_fallback,
                    module_declarations,
                    unresolved_property_macros,
                    role_provenance,
                    source,
                },
                functions,
                tests,
            },
        );
    }
    #[cfg(test)]
    pub(crate) fn insert_file_only(&mut self, path: PathBuf, facts: FileFacts) {
        self.insert_file(path, facts, false);
    }
    pub fn push_function(&mut self, fact: FunctionFact) {
        self.bump_membership_revision();
        let id = self.function_facts.allocate(fact);
        self.function_positions
            .resize(self.function_facts.len(), usize::MAX);
        self.function_positions[id.offset] = self.function_order.len();
        self.function_order.push(id);
    }
    pub fn push_test(&mut self, fact: TestFact) {
        self.bump_membership_revision();
        let id = self.test_facts.allocate(fact);
        self.test_positions
            .resize(self.test_facts.len(), usize::MAX);
        self.test_positions[id.offset] = self.test_order.len();
        self.test_order.push(id);
    }
    #[cfg(test)]
    pub fn remove_file(&mut self, path: &Path) -> bool {
        self.bump_membership_revision();
        self.files.remove(path).is_some()
    }
    /// Drop records unreachable from either view, preserving every surviving
    /// occurrence and each view's order. No source or earlier generation is pinned.
    pub(super) fn finalize(&mut self) -> Result<(), String> {
        crate::analysis::cancellation::checkpoint()?;
        self.validate_memberships()?;
        crate::analysis::cancellation::checkpoint()?;
        compact(
            &mut self.function_facts,
            &mut self.function_order,
            self.files.values_mut().map(|file| &mut file.functions),
        );
        crate::analysis::cancellation::checkpoint()?;
        compact(
            &mut self.test_facts,
            &mut self.test_order,
            self.files.values_mut().map(|file| &mut file.tests),
        );
        crate::analysis::cancellation::checkpoint()?;
        self.refresh_memberships()?;
        crate::analysis::cancellation::checkpoint()
    }
    pub(super) fn validate_memberships(&self) -> Result<(), String> {
        validate(
            &self.function_facts,
            self.function_order
                .iter()
                .chain(self.files.values().flat_map(|file| file.functions.iter())),
            "function",
        )?;
        validate(
            &self.test_facts,
            self.test_order
                .iter()
                .chain(self.files.values().flat_map(|file| file.tests.iter())),
            "test",
        )
    }
}

fn compact<'a, T: 'a>(
    arena: &mut FactArena<T>,
    flat: &mut [FactId<T>],
    local: impl Iterator<Item = &'a mut Vec<FactId<T>>>,
) {
    let mut memberships = local.collect::<Vec<_>>();
    let mut live = vec![false; arena.len()];
    for &id in flat
        .iter()
        .chain(memberships.iter().flat_map(|ids| ids.iter()))
    {
        live[id.offset] = true;
    }
    let mut remap = vec![0; arena.len()];
    let mut next = 0;
    for (id, &keep) in live.iter().enumerate() {
        if keep {
            remap[id] = next;
            next += 1;
        }
    }
    let mut old = 0;
    arena.records.retain(|_| {
        let keep = live[old];
        old += 1;
        keep
    });
    for id in flat
        .iter_mut()
        .chain(memberships.iter_mut().flat_map(|ids| ids.iter_mut()))
    {
        *id = FactId::new(remap[id.offset]);
    }
}

impl serde::Serialize for RustIndex {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("RustIndex", 10)?;
        state.serialize_field("files", &self.files())?;
        state.serialize_field("tests", &self.tests())?;
        state.serialize_field("functions", &self.functions())?;
        state.serialize_field("package_names", &self.package_names)?;
        state.serialize_field("include_parents", &self.include_parents)?;
        state.serialize_field("include_limitations", &self.include_limitations)?;
        state.serialize_field("non_utf8_sources", &self.non_utf8_sources)?;
        state.serialize_field("harness_subjects", &self.harness_subjects)?;
        state.serialize_field("harness_limitations", &self.harness_limitations)?;
        state.serialize_field("workspace_authority", &self.workspace_authority)?;
        state.end()
    }
}

impl Deref for IndexedFileFacts {
    type Target = FileData;
    fn deref(&self) -> &FileData {
        &self.data
    }
}
impl std::ops::DerefMut for IndexedFileFacts {
    fn deref_mut(&mut self) -> &mut FileData {
        &mut self.data
    }
}

impl RustIndex {
    /// Construct independent occurrence views supplied by callers. Unlike parser
    /// insertion, an expanded DTO carries no alias identity. Equal payloads in
    /// independent views are therefore not guessed to be the same occurrence.
    pub(crate) fn from_owned(owned: OwnedRustIndex) -> Self {
        let OwnedRustIndex {
            files,
            tests,
            functions,
            package_names,
            include_parents,
            include_limitations,
            non_utf8_sources,
            include_targets,
            harness_subjects,
            harness_limitations,
            workspace_authority,
        } = owned;
        let mut index = Self {
            package_names,
            include_parents,
            include_limitations,
            non_utf8_sources,
            include_targets,
            harness_subjects,
            harness_limitations,
            workspace_authority,
            ..Self::default()
        };
        for function in functions {
            index.push_function(function);
        }
        for test in tests {
            index.push_test(test);
        }
        for (path, facts) in files {
            index.insert_file(path, facts, false);
        }
        index
    }
    #[cfg(test)]
    pub(crate) fn owned_file(&self, path: &Path) -> Option<FileFacts> {
        let file = self.files().get(path)?;
        let FileData {
            path,
            calls,
            returns,
            literals,
            probe_shapes,
            used_lexical_fallback,
            module_declarations,
            unresolved_property_macros,
            role_provenance,
            source,
        } = file.data().clone();
        Some(FileFacts {
            path,
            functions: file.functions.iter().cloned().collect(),
            tests: file.tests.iter().cloned().collect(),
            calls,
            returns,
            literals,
            probe_shapes,
            used_lexical_fallback,
            module_declarations,
            unresolved_property_macros,
            role_provenance,
            source,
        })
    }
    #[cfg(test)]
    pub(crate) fn reverse_flat_membership(&mut self) -> Result<(), String> {
        self.function_order.reverse();
        self.test_order.reverse();
        self.refresh_memberships()
    }
    #[cfg(test)]
    pub(crate) fn function_at_mut(&mut self, position: usize) -> &mut FunctionFact {
        self.bump_membership_revision();
        &mut self.function_facts[self.function_order[position]]
    }
    #[cfg(test)]
    pub(crate) fn test_at_mut(&mut self, position: usize) -> &mut TestFact {
        self.bump_membership_revision();
        &mut self.test_facts[self.test_order[position]]
    }
    #[cfg(test)]
    pub(crate) fn replace_functions(&mut self, facts: Vec<FunctionFact>) {
        self.function_order.clear();
        self.function_positions.fill(usize::MAX);
        self.extend_functions(facts);
    }
    #[cfg(test)]
    pub(crate) fn replace_tests(&mut self, facts: Vec<TestFact>) {
        self.test_order.clear();
        self.test_positions.fill(usize::MAX);
        self.extend_tests(facts);
    }
    #[cfg(test)]
    pub fn file_data_mut(&mut self, path: &Path) -> Option<&mut FileData> {
        self.bump_membership_revision();
        self.files.get_mut(path).map(|file| &mut file.data)
    }
    #[cfg(test)]
    pub fn extend_functions(&mut self, facts: impl IntoIterator<Item = FunctionFact>) {
        for fact in facts {
            self.push_function(fact);
        }
    }
    #[cfg(test)]
    pub fn extend_tests(&mut self, facts: impl IntoIterator<Item = TestFact>) {
        for fact in facts {
            self.push_test(fact);
        }
    }
    /// Reconcile the existing flat and containing-file role selection laws.
    /// Aligned roles keep one record. Only a genuine semantic disagreement
    /// creates a distinct normalized flat occurrence; equal keys never alias
    /// unrelated records and ordinary parser-produced facts do not split.
    pub(super) fn apply_function_roles(
        &mut self,
        flat_updates: impl IntoIterator<Item = (FactId<FunctionFact>, FunctionSourceRole)>,
        local_updates: impl IntoIterator<Item = (FactId<FunctionFact>, FunctionSourceRole)>,
    ) {
        self.bump_membership_revision();
        let count = self.function_facts.len();
        let mut flat_roles = vec![None; count];
        let mut local_roles = vec![None; count];
        let mut in_flat = vec![false; count];
        let mut in_local = vec![false; count];
        for (id, role) in flat_updates {
            flat_roles[id.offset] = Some(role);
        }
        for (id, role) in local_updates {
            local_roles[id.offset] = Some(role);
        }
        for id in &self.function_order {
            in_flat[id.offset] = true;
        }
        for id in self.files.values().flat_map(|file| &file.functions) {
            in_local[id.offset] = true;
        }
        let mut splits = BTreeMap::new();
        for offset in 0..count {
            let id = FactId::new(offset);
            let original = self.function_facts[id].source_role;
            let flat = flat_roles[offset].unwrap_or(original);
            let local = local_roles[offset].unwrap_or(original);
            if in_flat[offset] && in_local[offset] && flat != local {
                let mut distinct = self.function_facts[id].clone();
                distinct.source_role = flat;
                let replacement = self.function_facts.allocate(distinct);
                splits.insert(id, replacement);
                if original != local {
                    self.function_facts[id].source_role = local;
                }
            } else {
                let role = if in_local[offset] {
                    local
                } else if in_flat[offset] {
                    flat
                } else {
                    continue;
                };
                if original != role {
                    self.function_facts[id].source_role = role;
                }
            }
        }
        if !splits.is_empty() {
            for id in &mut self.function_order {
                if let Some(&replacement) = splits.get(id) {
                    *id = replacement;
                }
            }
        }
    }
    pub(super) fn live_function_ids(&self) -> Vec<FactId<FunctionFact>> {
        let mut active = vec![false; self.function_facts.len()];
        for &id in self
            .function_order
            .iter()
            .chain(self.files.values().flat_map(|file| file.functions.iter()))
        {
            active[id.offset] = true;
        }
        active
            .into_iter()
            .enumerate()
            .filter(|(_, live)| *live)
            .map(|(offset, _)| FactId::new(offset))
            .collect()
    }
    pub(super) fn refresh_memberships(&mut self) -> Result<(), String> {
        crate::analysis::cancellation::checkpoint()?;
        self.validate_memberships()?;
        self.bump_membership_revision();
        self.function_positions.clear();
        self.function_positions
            .resize(self.function_facts.len(), usize::MAX);
        self.test_positions.clear();
        self.test_positions
            .resize(self.test_facts.len(), usize::MAX);
        for (position, id) in self.function_order.iter().enumerate().rev() {
            if position.is_multiple_of(1024) {
                crate::analysis::cancellation::checkpoint()?;
            }
            self.function_positions[id.offset] = position;
        }
        for (position, id) in self.test_order.iter().enumerate().rev() {
            if position.is_multiple_of(1024) {
                crate::analysis::cancellation::checkpoint()?;
            }
            self.test_positions[id.offset] = position;
        }
        Ok(())
    }
    fn bump_membership_revision(&mut self) {
        self.membership_revision = self.membership_revision.saturating_add(1);
    }
    pub(crate) fn storage_identity(&self) -> (usize, usize, u64) {
        (
            self.test_facts.as_ptr() as usize,
            self.function_facts.as_ptr() as usize,
            self.membership_revision,
        )
    }
    pub(crate) fn test_slot(&self, test: &TestFact) -> Option<usize> {
        if self.membership_revision == u64::MAX {
            return None;
        }
        slot_in(&self.test_facts, &self.test_positions, test)
    }
    pub(crate) fn function_slot(&self, function: &FunctionFact) -> Option<usize> {
        if self.membership_revision == u64::MAX {
            return None;
        }
        slot_in(&self.function_facts, &self.function_positions, function)
    }
    /// Mutate each reachable record exactly once, including flat-only and
    /// file-only records. The closure never has to mirror a write to a view.
    pub(crate) fn for_each_test_mut(&mut self, mut apply: impl FnMut(&mut TestFact)) {
        self.bump_membership_revision();
        let mut active = vec![false; self.test_facts.len()];
        for &id in self
            .test_order
            .iter()
            .chain(self.files.values().flat_map(|file| file.tests.iter()))
        {
            active[id.offset] = true;
        }
        for (fact, active) in self.test_facts.iter_mut().zip(active) {
            if active {
                apply(fact);
            }
        }
    }
}
impl<'de> serde::Deserialize<'de> for RustIndex {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let owned = OwnedRustIndex::deserialize(deserializer)?;
        let mut index = Self::from_owned(owned);
        index.finalize().map_err(serde::de::Error::custom)?;
        Ok(index)
    }
}

#[cfg(test)]
#[path = "index_tests.rs"]
mod tests;

// The existing run-scoped memo keys by flat occurrence, including detached
// rejection. Arena ownership changes the backing layout, not this identity law.
fn slot_in<T>(arena: &[T], positions: &[usize], item: &T) -> Option<usize> {
    let size = std::mem::size_of::<T>();
    if size == 0 {
        return None;
    }
    let offset = (item as *const T as usize).checked_sub(arena.as_ptr() as usize)?;
    if offset % size != 0 {
        return None;
    }
    let slot = offset / size;
    if !arena
        .get(slot)
        .is_some_and(|candidate| std::ptr::eq(candidate, item))
    {
        return None;
    }
    positions
        .get(slot)
        .copied()
        .filter(|&position| position != usize::MAX)
}
