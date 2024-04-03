//! Crate to write a Unicode tree.
//!
//! Modified from: <https://docs.rs/ascii_tree/0.1.1/src/ascii_tree/lib.rs.html>

use std::fmt::Display;
use std::sync::Arc;

use parking_lot::Mutex;

#[derive(Clone)]
pub struct Tree {
    label: Vec<String>,
    pub children: Vec<Arc<Mutex<Tree>>>,
}

impl Tree {
    pub fn leaf(label: Vec<String>) -> Self {
        Self {
            label,
            children: Vec::new(),
        }
    }

    #[cfg(test)]
    pub fn new_from(label: impl AsRef<str>, children: impl IntoIterator<Item = Tree>) -> Self {
        Self {
            label: label.as_ref().lines().map(|line| line.to_owned()).collect(),
            children: children
                .into_iter()
                .map(|child| Arc::new(Mutex::new(child)))
                .collect(),
        }
    }

    #[cfg(test)]
    pub fn leaf_from(label: impl AsRef<str>) -> Self {
        Self::new_from(label, [])
    }
}

impl Display for Tree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write_tree_element(f, self, &mut vec![])
    }
}

fn write_tree_element(
    f: &mut std::fmt::Formatter<'_>,
    tree: &Tree,
    level: &mut Vec<usize>,
) -> std::fmt::Result {
    const EMPTY: &str = "  ";
    const EDGE: &str = "└─";
    const PIPE: &str = "│ ";
    const BRANCH: &str = "├─";

    let maxpos = level.len();
    let mut second_line = String::new();
    for (pos, l) in level.iter().enumerate() {
        let prefix: &str = if pos == 0 { "" } else { " " };
        let last_row = pos == maxpos - 1;
        second_line.push_str(prefix);
        if *l == 1 {
            if !last_row {
                write!(f, "{prefix}{EMPTY}")?
            } else {
                write!(f, "{prefix}{EDGE}")?
            }
            second_line.push_str(EMPTY);
        } else {
            if !last_row {
                write!(f, "{prefix}{PIPE}")?
            } else {
                write!(f, "{prefix}{BRANCH}")?
            }
            second_line.push_str(PIPE);
        }
    }

    let prefix: &str = if maxpos == 0 { "" } else { " " };
    for (i, s) in tree.label.iter().enumerate() {
        match i {
            0 => writeln!(f, "{prefix}{s}")?,
            _ => writeln!(f, "{second_line}{prefix}{s}")?,
        }
    }

    let mut children_remaining = tree.children.len();
    for s in &tree.children {
        level.push(children_remaining);
        children_remaining -= 1;
        write_tree_element(f, &s.lock(), level)?;
        level.pop();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_tree_display() {
        assert_eq!(
            Tree::new_from(
                "a",
                [
                    Tree::new_from("b\nc", [Tree::leaf_from("d")]),
                    Tree::leaf_from("e")
                ]
            )
            .to_string(),
            indoc!(
                "
                a
                ├─ b
                │  c
                │  └─ d
                └─ e
                "
            )
        );
    }

    #[test]
    fn test_tree_display_multi_line() {
        assert_eq!(
            Tree::new_from(
                "a\nb\nc",
                [
                    Tree::new_from("d\ne", [Tree::leaf_from("f\ng")]),
                    Tree::leaf_from("h\ni"),
                    Tree::leaf_from("j\nk")
                ]
            )
            .to_string(),
            indoc!(
                "
                a
                b
                c
                ├─ d
                │  e
                │  └─ f
                │     g
                ├─ h
                │  i
                └─ j
                   k
                "
            )
        );
    }
}
