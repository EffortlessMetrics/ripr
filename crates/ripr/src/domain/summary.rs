use crate::domain::ExposureClass;

/// Changed-file count attributed to one language adapter run.
///
/// `language` is the stable wire string from `LanguageId::as_str`
/// (`"rust"`, `"typescript"`, `"javascript"`, `"python"`, `"perl"`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LanguageFileCount {
    pub language: String,
    pub files: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    /// Changed files attributed to the Rust adapter only (#2103). Preview
    /// language adapters report their own counts in
    /// `changed_files_by_language`; they must never inflate this field.
    pub changed_rust_files: usize,
    /// Per-language changed-file counts, one entry per adapter that ran,
    /// sorted by language wire string for deterministic serialization.
    pub changed_files_by_language: Vec<LanguageFileCount>,
    pub probes: usize,
    pub findings: usize,
    pub exposed: usize,
    pub weakly_exposed: usize,
    pub reachable_unrevealed: usize,
    pub no_static_path: usize,
    pub infection_unknown: usize,
    pub propagation_unknown: usize,
    pub static_unknown: usize,
}

impl Summary {
    /// Record `files` changed files for `language`, merging into an existing
    /// entry when the adapter already reported one. Entries stay sorted by
    /// language wire string so JSON output order is deterministic.
    pub fn record_changed_files_by_language(&mut self, language: &str, files: usize) {
        match self
            .changed_files_by_language
            .iter_mut()
            .find(|count| count.language == language)
        {
            Some(count) => count.files += files,
            None => {
                self.changed_files_by_language.push(LanguageFileCount {
                    language: language.to_string(),
                    files,
                });
                self.changed_files_by_language
                    .sort_by(|a, b| a.language.cmp(&b.language));
            }
        }
    }

    pub fn increment_exposure_class(&mut self, class: &ExposureClass) {
        match class {
            ExposureClass::Exposed => self.exposed += 1,
            ExposureClass::WeaklyExposed => self.weakly_exposed += 1,
            ExposureClass::ReachableUnrevealed => self.reachable_unrevealed += 1,
            ExposureClass::NoStaticPath => self.no_static_path += 1,
            ExposureClass::InfectionUnknown => self.infection_unknown += 1,
            ExposureClass::PropagationUnknown => self.propagation_unknown += 1,
            ExposureClass::StaticUnknown => self.static_unknown += 1,
        }
    }

    /// Removes one finding of `class` from its per-class bucket. Used when a
    /// `--suppression-policy` suppresses a finding (#1441): the per-class
    /// buckets count unsuppressed findings only, while `findings` stays the
    /// total rendered count. Saturating so a policy bug can never underflow.
    pub fn decrement_exposure_class(&mut self, class: &ExposureClass) {
        let bucket = match class {
            ExposureClass::Exposed => &mut self.exposed,
            ExposureClass::WeaklyExposed => &mut self.weakly_exposed,
            ExposureClass::ReachableUnrevealed => &mut self.reachable_unrevealed,
            ExposureClass::NoStaticPath => &mut self.no_static_path,
            ExposureClass::InfectionUnknown => &mut self.infection_unknown,
            ExposureClass::PropagationUnknown => &mut self.propagation_unknown,
            ExposureClass::StaticUnknown => &mut self.static_unknown,
        };
        *bucket = bucket.saturating_sub(1);
    }
}

#[cfg(test)]
mod tests {
    use super::Summary;
    use crate::domain::ExposureClass;

    #[test]
    fn default_summary_starts_with_zero_counts() {
        let summary = Summary::default();

        assert_eq!(summary.changed_rust_files, 0);
        assert!(summary.changed_files_by_language.is_empty());
        assert_eq!(summary.probes, 0);
        assert_eq!(summary.findings, 0);
        assert_eq!(summary.exposed, 0);
        assert_eq!(summary.weakly_exposed, 0);
        assert_eq!(summary.reachable_unrevealed, 0);
        assert_eq!(summary.no_static_path, 0);
        assert_eq!(summary.infection_unknown, 0);
        assert_eq!(summary.propagation_unknown, 0);
        assert_eq!(summary.static_unknown, 0);
    }

    #[test]
    fn record_changed_files_by_language_merges_and_stays_sorted() {
        let mut summary = Summary::default();

        summary.record_changed_files_by_language("rust", 2);
        summary.record_changed_files_by_language("python", 5);
        summary.record_changed_files_by_language("rust", 1);

        let entries: Vec<(&str, usize)> = summary
            .changed_files_by_language
            .iter()
            .map(|count| (count.language.as_str(), count.files))
            .collect();
        assert_eq!(entries, vec![("python", 5), ("rust", 3)]);
    }

    #[test]
    fn increment_exposure_class_counts_each_class() {
        let mut summary = Summary::default();

        let classes = [
            ExposureClass::Exposed,
            ExposureClass::WeaklyExposed,
            ExposureClass::ReachableUnrevealed,
            ExposureClass::NoStaticPath,
            ExposureClass::InfectionUnknown,
            ExposureClass::PropagationUnknown,
            ExposureClass::StaticUnknown,
        ];

        for class in classes {
            summary.increment_exposure_class(&class);
        }

        assert_eq!(summary.exposed, 1);
        assert_eq!(summary.weakly_exposed, 1);
        assert_eq!(summary.reachable_unrevealed, 1);
        assert_eq!(summary.no_static_path, 1);
        assert_eq!(summary.infection_unknown, 1);
        assert_eq!(summary.propagation_unknown, 1);
        assert_eq!(summary.static_unknown, 1);
    }

    #[test]
    fn decrement_exposure_class_reverses_increment_and_saturates_at_zero() {
        let mut summary = Summary::default();

        summary.increment_exposure_class(&ExposureClass::WeaklyExposed);
        summary.decrement_exposure_class(&ExposureClass::WeaklyExposed);
        assert_eq!(summary.weakly_exposed, 0);

        // Saturating: a policy bug must never underflow a bucket.
        summary.decrement_exposure_class(&ExposureClass::WeaklyExposed);
        assert_eq!(summary.weakly_exposed, 0);
    }

    #[test]
    fn increment_exposure_class_accumulates_repeated_classes() {
        let mut summary = Summary::default();

        summary.increment_exposure_class(&ExposureClass::NoStaticPath);
        summary.increment_exposure_class(&ExposureClass::NoStaticPath);
        summary.increment_exposure_class(&ExposureClass::NoStaticPath);

        assert_eq!(summary.no_static_path, 3);
    }
}
