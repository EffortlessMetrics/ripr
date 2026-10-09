//! Persistent extraction over one original semantic file and its whole-file index.
//!
//! Only probe construction is lazy. Selected shape/use metadata and prepared
//! whole-file context are retained; this is not a memory or raw-coverage proof.
use super::super::binding_predicate::ChangedBindingPredicateUse;
use super::super::classify::ParserProbeShape;
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AdvanceState {
    Exhausted,
    Paused,
    Poisoned,
    InvalidQuota,
}

enum Pending<'a> {
    Empty,
    AddedFamilies {
        line: usize,
        families: std::vec::IntoIter<ProbeFamily>,
    },
    AddedShapes {
        line: usize,
        canonical: bool,
        shapes: std::vec::IntoIter<ParserProbeShape<'a>>,
    },
    Retarget {
        before: Option<String>,
        uses: std::vec::IntoIter<ChangedBindingPredicateUse>,
    },
    RemovedFamilies {
        line: usize,
        families: std::vec::IntoIter<ProbeFamily>,
    },
}

pub(super) struct RustProbeCursor<'a> {
    root: &'a Path,
    changed: &'a ChangedFile,
    index: &'a RustIndex,
    changed_lines: Vec<usize>,
    changed_nodes: Vec<ChangedOwnerSpan>,
    test_module_ranges: Vec<InlineModuleRange>,
    skip_added: Vec<bool>,
    skip_removed: Vec<bool>,
    added: usize,
    removed: usize,
    pending: Pending<'a>,
    emitted_parser_shapes: Vec<(usize, String)>,
    seen: std::collections::HashMap<String, u32>,
    poisoned: bool,
}

impl<'a> RustProbeCursor<'a> {
    pub(super) fn new(root: &'a Path, changed: &'a ChangedFile, index: &'a RustIndex) -> Self {
        let changed_lines = changed
            .added_lines
            .iter()
            .chain(changed.removed_lines.iter())
            .map(|line| line.new_side_line)
            .collect::<Vec<_>>();
        Self {
            root,
            changed,
            index,
            changed_nodes: changed_owner_spans_for_lines(index, &changed.path, &changed_lines),
            changed_lines,
            test_module_ranges: test_module_ranges_for(index, &changed.path),
            skip_added: structural_lines_covered_by_run(&changed.added_lines),
            skip_removed: structural_lines_covered_by_run(&changed.removed_lines),
            added: 0,
            removed: 0,
            pending: Pending::Empty,
            emitted_parser_shapes: Vec::new(),
            seen: std::collections::HashMap::new(),
            poisoned: false,
        }
    }

    /// Quotas bound new input selections and emissions independently. An
    /// expansion may span calls without reselecting its input. These states
    /// cannot certify raw obligations, classification or complete coverage.
    pub(super) fn advance<E>(
        &mut self,
        input_quota: usize,
        probe_quota: usize,
        mut emit: impl FnMut(SeededProbe) -> Result<(), E>,
    ) -> Result<AdvanceState, E> {
        if self.poisoned {
            return Ok(AdvanceState::Poisoned);
        }
        if input_quota == 0 || probe_quota == 0 {
            return Ok(AdvanceState::InvalidQuota);
        }
        let mut inputs = 0;
        let mut probes = 0;
        while probes < probe_quota {
            if matches!(self.pending, Pending::Empty) {
                if self.added == self.changed.added_lines.len()
                    && self.removed == self.changed.removed_lines.len()
                {
                    return Ok(AdvanceState::Exhausted);
                }
                if inputs == input_quota {
                    return Ok(AdvanceState::Paused);
                }
                self.prepare_next_input();
                inputs += 1;
            }
            if let Some(mut seeded) = self.next_pending_probe() {
                dedup_probe_id(&mut seeded, &mut self.seen);
                if let Err(error) = emit(seeded) {
                    self.poisoned = true;
                    return Err(error);
                }
                probes += 1;
            }
        }
        Ok(AdvanceState::Paused)
    }

    fn prepare_next_input(&mut self) {
        if self.added < self.changed.added_lines.len() {
            let line = self.added;
            self.added += 1;
            let added = &self.changed.added_lines[line];
            let text = added.text.trim();
            if should_ignore_changed_line(text)
                || self.skip_added[line]
                || changed_line_is_test_evidence(
                    self.index,
                    &self.changed.path,
                    added.new_side_line,
                    &self.test_module_ranges,
                )
                || opens_new_function_with_added_body(
                    self.index,
                    self.changed,
                    added.new_side_line,
                    text,
                )
            {
                return;
            }
            if let Some(family) = bounded_subprocess_family(
                self.index,
                &self.changed.path,
                added.new_side_line,
                text,
            ) {
                self.pending = Pending::AddedFamilies {
                    line,
                    families: vec![family].into_iter(),
                };
                return;
            }
            let shapes = parser_probe_shapes_for_changed_line(
                self.index,
                &self.changed.path,
                added.new_side_line,
                text,
            )
            .into_iter()
            .filter(|shape| shape.family != ProbeFamily::CallDeletion || shape.standalone_call)
            .collect::<Vec<_>>();
            let canonical = shapes
                .iter()
                .filter(|shape| {
                    self.changed
                        .added_lines
                        .iter()
                        .any(|line| line.new_side_line == shape.start_line)
                })
                .cloned()
                .collect::<Vec<_>>();
            if !canonical.is_empty() {
                self.pending = Pending::AddedShapes {
                    line,
                    canonical: true,
                    shapes: canonical.into_iter(),
                };
                return;
            }
            if !shapes.is_empty() {
                self.pending = Pending::AddedShapes {
                    line,
                    canonical: false,
                    shapes: shapes.into_iter(),
                };
                return;
            }
            let families = classify_changed_line(text);
            if families == [ProbeFamily::StaticUnknown]
                && let Some((before, uses)) = self.prepare_retarget(added, text)
            {
                self.pending = Pending::Retarget {
                    before,
                    uses: uses.into_iter(),
                };
                return;
            }
            self.pending = Pending::AddedFamilies {
                line,
                families: families.into_iter(),
            };
        } else if self.removed < self.changed.removed_lines.len() {
            let line = self.removed;
            self.removed += 1;
            let removed = &self.changed.removed_lines[line];
            let text = removed.text.trim();
            if should_ignore_changed_line(text)
                || self.skip_removed[line]
                || changed_line_is_test_evidence(
                    self.index,
                    &self.changed.path,
                    removed.new_side_line,
                    &self.test_module_ranges,
                )
            {
                return;
            }
            self.pending = Pending::RemovedFamilies {
                line,
                families: classify_changed_line(text).into_iter(),
            };
        }
    }

    fn prepare_retarget(
        &self,
        added: &ChangedLine,
        text: &str,
    ) -> Option<(Option<String>, Vec<ChangedBindingPredicateUse>)> {
        let (binding, initializer) = changed_let_binding(text)?;
        if !text.ends_with(';') || masked_paren_delta(text) != 0 || masked_brace_delta(text) != 0 {
            return None;
        }
        let owner = find_owner_function(self.index, &self.changed.path, added.new_side_line)?;
        let uses = match resolve_changed_binding_uses(
            binding,
            initializer,
            &owner.body,
            owner.start_line,
            added.new_side_line,
        ) {
            BindingPredicateResolution::DirectUses(uses) => uses
                .into_iter()
                .filter(|use_site| !self.changed_lines.contains(&use_site.predicate_line))
                .collect::<Vec<_>>(),
            _ => return None,
        };
        if uses.is_empty() {
            return None;
        }
        let before = nearby_removed_line(added.new_side_line, text, self.changed)
            .and_then(|removed_text| {
                changed_let_binding(&removed_text).map(|(removed_binding, removed_initializer)| {
                    (removed_binding.to_string(), removed_initializer.to_string())
                })
            })
            .and_then(|(removed_binding, removed_initializer)| {
                (removed_binding == binding).then_some(removed_initializer)
            });
        Some((before, uses))
    }

    fn next_pending_probe(&mut self) -> Option<SeededProbe> {
        let context = ProbeBuildContext {
            root: self.root,
            changed: self.changed,
            index: self.index,
            changed_nodes: &self.changed_nodes,
        };
        let seeded = match &mut self.pending {
            Pending::Empty => None,
            Pending::AddedFamilies { line, families } => families.next().map(|family| {
                let added = &self.changed.added_lines[*line];
                let text = added.text.trim();
                SeededProbe::from_probe(build_probe(
                    &context,
                    added,
                    family,
                    nearby_removed_line(added.new_side_line, text, self.changed),
                    Some(text.to_string()),
                ))
            }),
            Pending::AddedShapes {
                line,
                canonical,
                shapes,
            } => {
                let added = &self.changed.added_lines[*line];
                let text = added.text.trim();
                let mut next = None;
                for shape in shapes {
                    if *canonical {
                        let key = (shape.start_byte, shape.family.as_str().to_string());
                        if self.emitted_parser_shapes.iter().any(|current| current == &key) {
                            continue;
                        }
                        self.emitted_parser_shapes.push(key);
                        let canonical_text = canonical_probe_text(text, shape.text);
                        let span = parser_span_for_canonical_shape(&canonical_text, &shape);
                        let canonical_line = ChangedLine {
                            line: shape.start_line,
                            new_side_line: shape.start_line,
                            text: canonical_text.clone(),
                        };
                        next = Some(SeededProbe::maybe_with_span(
                            build_probe(
                                &context,
                                &canonical_line,
                                shape.family,
                                nearby_removed_line(shape.start_line, &canonical_text, self.changed),
                                Some(canonical_text),
                            ),
                            span,
                        ));
                    } else {
                        next = Some(SeededProbe::from_probe(build_probe(
                            &context,
                            added,
                            shape.family,
                            nearby_removed_line(added.new_side_line, text, self.changed),
                            Some(text.to_string()),
                        )));
                    }
                    break;
                }
                next
            }
            Pending::Retarget { before, uses } => uses.next().map(|use_site| {
                let predicate_line = ChangedLine {
                    line: use_site.predicate_line,
                    new_side_line: use_site.predicate_line,
                    text: use_site.predicate_expression.clone(),
                };
                SeededProbe::retargeted(
                    build_probe(
                        &context,
                        &predicate_line,
                        ProbeFamily::Predicate,
                        before.clone(),
                        Some(use_site.initializer.clone()),
                    ),
                    use_site,
                )
            }),
            Pending::RemovedFamilies { line, families } => {
                let removed = &self.changed.removed_lines[*line];
                let text = removed.text.trim();
                let mut next = None;
                for family in families {
                    if has_matching_added_line(removed, &family, self.changed) {
                        continue;
                    }
                    next = Some(SeededProbe::from_probe(build_probe(
                        &context,
                        removed,
                        family,
                        Some(text.to_string()),
                        None,
                    )));
                    break;
                }
                next
            }
        };
        if seeded.is_none() {
            self.pending = Pending::Empty;
        }
        seeded
    }
}
