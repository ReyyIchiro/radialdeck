use crate::folder::Folder;

/// Ranking tier for one query match: 0 = label starts with the query,
/// 1 = label contains it, 2 = only the path contains it, `None` = no match.
/// An empty (or whitespace) query matches everything at tier 0.
///
/// Shared by every fuzzy filter so the folder wheel and future UIs rank the
/// same way; [`filter`] is the whole-list convenience wrapper over it.
pub fn match_rank(label: &str, path: &str, query: &str) -> Option<u8> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Some(0);
    }
    let label = label.to_lowercase();
    let path = path.to_lowercase();
    if label.starts_with(&query) {
        Some(0)
    } else if label.contains(&query) {
        Some(1)
    } else if path.contains(&query) {
        Some(2)
    } else {
        None
    }
}

/// Case-insensitive filter with stable ranking:
/// label prefix, then label contains, then path contains.
pub fn filter<'a>(folders: &'a [Folder], query: &str) -> Vec<&'a Folder> {
    let mut matches: Vec<(u8, usize, &Folder)> = folders
        .iter()
        .enumerate()
        .filter_map(|(index, folder)| {
            match_rank(&folder.label, &folder.path.to_string_lossy(), query)
                .map(|rank| (rank, index, folder))
        })
        .collect();

    matches.sort_by_key(|(rank, index, _)| (*rank, *index));
    matches.into_iter().map(|(_, _, folder)| folder).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn folder(label: &str, path: &str) -> Folder {
        Folder::new(PathBuf::from(path), label, None)
    }

    #[test]
    fn empty_query_keeps_order() {
        let folders = vec![folder("Beta", "/b"), folder("Alpha", "/a")];
        let hits = filter(&folders, "  ");
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].label, "Beta");
    }

    #[test]
    fn ranks_prefix_before_substring_before_path() {
        let folders = vec![
            folder("My Website", "/work/site"),
            folder("Website", "/work/website"),
            folder("Archive", "/backup/website-old"),
        ];

        let hits = filter(&folders, "web");
        assert_eq!(
            hits.iter().map(|f| f.label.as_str()).collect::<Vec<_>>(),
            vec!["Website", "My Website", "Archive"]
        );
    }

    #[test]
    fn matching_is_case_insensitive() {
        let folders = vec![folder("ReyyIchiro", "/code/ReyyIchiro")];
        assert_eq!(filter(&folders, "reyy").len(), 1);
        assert_eq!(filter(&folders, "ICHIRO").len(), 1);
        assert!(filter(&folders, "nope").is_empty());
    }

    #[test]
    fn match_rank_matches_path_only_hits() {
        assert_eq!(match_rank("Deck", "/code/radial-deck", "radial"), Some(2));
        assert_eq!(match_rank("Deck", "/code/radial-deck", "DEC"), Some(0));
        assert_eq!(match_rank("Deck", "/code/other", "zzz"), None);
        assert_eq!(match_rank("Deck", "/code/other", "   "), Some(0));
    }
}
