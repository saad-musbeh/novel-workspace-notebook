//! Range editing on a [`Document`]: insert text, split paragraphs, delete ranges across
//! paragraphs and tables, apply character/paragraph formatting, copy a range to a fragment and
//! paste a fragment.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::para::InlineObject;
use crate::props::{CharProps, ParaProps};
use crate::{Block, Blocks, DocError, Document, Paragraph, Path, Pos, Result, para_block};

/// A copied piece of a document: blocks, where the first and last paragraphs may be partial.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Fragment {
    pub blocks: Vec<Block>,
}

impl Fragment {
    pub fn plain_text(&self) -> String {
        let mut d = Document::new();
        d.body = self.blocks.iter().cloned().map(Arc::new).collect();
        d.plain_text(crate::StoryRef::Body)
    }
    pub fn from_text(s: &str) -> Fragment {
        let lines: Vec<&str> = s.split('\n').collect();
        Fragment { blocks: lines.iter().map(|l| Block::Para(Paragraph::with_text(l.trim_end_matches('\r'), CharProps::default()))).collect() }
    }
}

fn order(a: &Pos, b: &Pos) -> (Pos, Pos) {
    if a <= b { (a.clone(), b.clone()) } else { (b.clone(), a.clone()) }
}

impl Document {
    /// Insert plain text at `pos` (`\n` inside `s` is a manual line break; use
    /// [`Document::split_paragraph`] for paragraph breaks). Returns the position after it.
    pub fn insert_text(&mut self, pos: &Pos, s: &str, props: &CharProps) -> Result<Pos> {
        let p = self.para_mut(pos.story, &pos.path)?;
        let n = p.insert_text(pos.off, s, props)?;
        Ok(Pos { off: pos.off + n, ..pos.clone() })
    }

    /// Insert an inline object at `pos`; returns the position after it.
    pub fn insert_object(&mut self, pos: &Pos, obj: InlineObject, props: &CharProps) -> Result<Pos> {
        let p = self.para_mut(pos.story, &pos.path)?;
        p.insert_object(pos.off, obj, props)?;
        Ok(Pos { off: pos.off + crate::para::OBJ.len_utf8(), ..pos.clone() })
    }

    /// Split the paragraph at `pos`; returns the start of the new (second) paragraph.
    pub fn split_paragraph(&mut self, pos: &Pos) -> Result<Pos> {
        let i = pos.path.last();
        let tail = self.para_mut(pos.story, &pos.path)?.split_off(pos.off)?;
        let bl = self.container_mut(pos.story, &pos.path)?;
        bl.insert((i + 1).min(bl.len()), para_block(tail));
        Ok(Pos { story: pos.story, path: pos.path.with_last(i + 1), off: 0 })
    }

    /// Insert a block before the block at `path` (same container).
    pub fn insert_block(&mut self, story: crate::StoryRef, path: &Path, block: Block) -> Result<()> {
        let i = path.last();
        let bl = self.container_mut(story, path)?;
        if i > bl.len() {
            return Err(DocError::BadPath(path.to_string()));
        }
        bl.insert(i, Arc::new(block));
        Ok(())
    }

    /// Remove the block at `path` (keeps at least one paragraph in the container).
    pub fn remove_block(&mut self, story: crate::StoryRef, path: &Path) -> Result<Block> {
        let i = path.last();
        let bl = self.container_mut(story, path)?;
        if i >= bl.len() {
            return Err(DocError::BadPath(path.to_string()));
        }
        let b = bl.remove(i);
        if bl.is_empty() || matches!(bl.last().map(|b| &**b), Some(Block::Table(_))) {
            bl.push(para_block(Paragraph::new()));
        }
        Ok(Arc::try_unwrap(b).unwrap_or_else(|a| (*a).clone()))
    }

    /// Paragraph paths between `a` and `b` inclusive (same story), document order.
    pub fn paths_between(&self, a: &Pos, b: &Pos) -> Vec<Path> {
        let (a, b) = order(a, b);
        if a.story != b.story {
            return Vec::new();
        }
        self.para_paths(a.story).into_iter().filter(|p| *p >= a.path && *p <= b.path).collect()
    }

    /// Delete `a..b`. Returns the position where the deletion collapsed to.
    pub fn delete_range(&mut self, a: &Pos, b: &Pos) -> Result<Pos> {
        let (a, b) = order(a, b);
        if a == b {
            return Ok(a);
        }
        if a.story != b.story {
            return Err(DocError::Invalid("range spans stories".into()));
        }
        if a.path == b.path {
            self.para_mut(a.story, &a.path)?.delete(a.off, b.off)?;
            return Ok(a);
        }
        if a.path.parent() == b.path.parent() {
            // Same container: trim the ends, drop the blocks between, join.
            let (ia, ib) = (a.path.last(), b.path.last());
            {
                let pa = self.para_mut(a.story, &a.path)?;
                let end = pa.len();
                pa.delete(a.off, end)?;
            }
            let mut tail = {
                let pb = self.para_mut(b.story, &b.path)?;
                pb.delete(0, b.off)?;
                pb.clone()
            };
            let bl = self.container_mut(a.story, &a.path)?;
            if ib < bl.len() && ia < ib {
                bl.drain(ia + 1..=ib);
            }
            tail.props = ParaProps::default();
            let pa = self.para_mut(a.story, &a.path)?;
            let keep_props = pa.props.clone();
            pa.append(tail);
            pa.props = keep_props;
            return Ok(a);
        }
        // Different containers (range enters or leaves a table): clear text in every paragraph
        // inside; remove whole tables that are entirely covered when at the same level as `a`.
        let paths = self.paths_between(&a, &b);
        for p in paths.iter().rev() {
            let (from, to) = {
                let Some(para) = self.para(a.story, p) else { continue };
                let from = if *p == a.path { a.off } else { 0 };
                let to = if *p == b.path { b.off } else { para.len() };
                (from, to)
            };
            self.para_mut(a.story, p)?.delete(from, to)?;
        }
        // Drop top-level tables strictly between a and b at a's level.
        if a.path.depth() == 0 && b.path.depth() > 0 {
            let first_b = b.path.0.first().copied().unwrap_or(0) as usize;
            let ia = a.path.last();
            let bl = self.container_mut(a.story, &a.path)?;
            let mut i = first_b;
            while i > ia + 1 {
                i -= 1;
                if matches!(bl.get(i).map(|b| &**b), Some(Block::Table(_))) {
                    bl.remove(i);
                }
            }
        }
        Ok(a)
    }

    /// Apply `f` to the character formatting of `a..b`. A collapsed range changes nothing (the
    /// engine keeps "pending" formatting for the caret instead).
    pub fn format_range(&mut self, a: &Pos, b: &Pos, f: &dyn Fn(&mut CharProps)) -> Result<()> {
        let (a, b) = order(a, b);
        for p in self.paths_between(&a, &b) {
            let para = self.para_mut(a.story, &p)?;
            let from = if p == a.path { a.off } else { 0 };
            let to = if p == b.path { b.off } else { para.len() };
            para.format(from, to, f)?;
            // A fully selected paragraph also gets the mark formatted (Word does).
            if to == para.len() && (from == 0 || p != a.path) {
                f(&mut para.mark);
            }
        }
        Ok(())
    }

    /// Apply `f` to the paragraph properties of every paragraph touched by `a..b`.
    pub fn format_paragraphs(&mut self, a: &Pos, b: &Pos, f: &dyn Fn(&mut ParaProps)) -> Result<()> {
        for p in self.paths_between(a, b) {
            let para = self.para_mut(a.story, &p)?;
            f(&mut para.props);
            para.touch();
        }
        Ok(())
    }

    /// Copy `a..b` into a fragment.
    pub fn copy_range(&self, a: &Pos, b: &Pos) -> Fragment {
        let (a, b) = order(a, b);
        let mut out = Vec::new();
        if a.story != b.story {
            return Fragment::default();
        }
        if a.path.parent() == b.path.parent() {
            let Some(bl) = self.container(a.story, &a.path) else { return Fragment::default() };
            for i in a.path.last()..=b.path.last() {
                let Some(blk) = bl.get(i) else { break };
                match &**blk {
                    Block::Para(p) => {
                        let mut p = p.clone();
                        let to = if i == b.path.last() { b.off } else { p.len() };
                        let from = if i == a.path.last() { a.off } else { 0 };
                        let _ = p.delete(to.min(p.len()), p.len());
                        let _ = p.delete(0, from.min(p.len()));
                        if i == b.path.last() && to < self.para(a.story, &b.path).map(Paragraph::len).unwrap_or(0) {
                            p.section = None;
                        }
                        out.push(Block::Para(p));
                    }
                    Block::Table(t) => out.push(Block::Table(t.clone())),
                }
            }
            return Fragment { blocks: out };
        }
        for p in self.paths_between(&a, &b) {
            if let Some(para) = self.para(a.story, &p) {
                let mut q = para.clone();
                let to = if p == b.path { b.off } else { q.len() };
                let from = if p == a.path { a.off } else { 0 };
                let _ = q.delete(to.min(q.len()), q.len());
                let _ = q.delete(0, from.min(q.len()));
                q.section = None;
                out.push(Block::Para(q));
            }
        }
        Fragment { blocks: out }
    }

    /// Insert a fragment at `pos`; returns the position after the pasted content.
    pub fn insert_fragment(&mut self, pos: &Pos, frag: &Fragment) -> Result<Pos> {
        let mut blocks = frag.blocks.clone();
        if blocks.is_empty() {
            return Ok(pos.clone());
        }
        // Single paragraph: inline insert keeping runs.
        if blocks.len() == 1
            && let Some(Block::Para(src)) = blocks.first()
        {
            let src = src.clone();
            let target = self.para_mut(pos.story, &pos.path)?;
            let tail = target.split_off(pos.off)?;
            let added = src.len();
            let keep = target.props.clone();
            let sect = tail.section.clone();
            let mut src = src;
            src.section = None;
            target.append(src);
            target.append(tail);
            target.props = keep;
            target.section = sect;
            return Ok(Pos { off: pos.off + added, ..pos.clone() });
        }
        // Multi-block: split, merge first into left part, last into right part.
        let after = self.split_paragraph(pos)?;
        let left = pos.path.clone();
        let first = blocks.remove(0);
        let last = blocks.pop();
        let mut idx = left.last() + 1;
        match first {
            Block::Para(p) => {
                let l = self.para_mut(pos.story, &left)?;
                let keep = l.props.clone();
                let mut p2 = p;
                let pprops = p2.props.clone();
                p2.section = None;
                l.append(p2);
                l.props = if keep.is_empty() { pprops } else { keep };
            }
            t @ Block::Table(_) => {
                self.insert_block(pos.story, &left.with_last(idx), t)?;
                idx += 1;
            }
        }
        for b in blocks {
            self.insert_block(pos.story, &left.with_last(idx), b)?;
            idx += 1;
        }
        let right = after.path.with_last(idx);
        match last {
            Some(Block::Para(p)) => {
                let r = self.para_mut(pos.story, &right)?;
                let tail = std::mem::replace(r, p.clone());
                let off = r.len();
                let sect = tail.section.clone();
                r.append(tail);
                r.section = sect;
                Ok(Pos { story: pos.story, path: right, off })
            }
            Some(t @ Block::Table(_)) => {
                self.insert_block(pos.story, &right, t)?;
                Ok(Pos { story: pos.story, path: right.with_last(idx + 1), off: 0 })
            }
            None => Ok(Pos { story: pos.story, path: right, off: 0 }),
        }
    }

    /// Replace every block of a story.
    pub fn set_story(&mut self, story: crate::StoryRef, blocks: Blocks) -> Result<()> {
        let s = self.story_mut(story)?;
        *s = blocks;
        if s.is_empty() {
            s.push(para_block(Paragraph::new()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{StoryRef, Table};

    fn doc(lines: &[&str]) -> Document {
        Document::from_text(&lines.join("\n"))
    }

    #[test]
    fn split_and_join() {
        let mut d = doc(&["Hello world"]);
        let p = d.split_paragraph(&Pos::body(0, 5)).unwrap();
        assert_eq!(p, Pos::body(1, 0));
        assert_eq!(d.plain_text(StoryRef::Body), "Hello\n world");
        let at = d.delete_range(&Pos::body(0, 5), &Pos::body(1, 0)).unwrap();
        assert_eq!(at, Pos::body(0, 5));
        assert_eq!(d.plain_text(StoryRef::Body), "Hello world");
    }

    #[test]
    fn delete_multi() {
        let mut d = doc(&["one", "two", "three"]);
        d.delete_range(&Pos::body(2, 2), &Pos::body(0, 1)).unwrap();
        assert_eq!(d.plain_text(StoryRef::Body), "oree");
        assert_eq!(d.body.len(), 1);
    }

    #[test]
    fn format_across() {
        let mut d = doc(&["abc", "def"]);
        d.format_range(&Pos::body(0, 1), &Pos::body(1, 2), &|c| c.bold = Some(true)).unwrap();
        let p0 = d.para(StoryRef::Body, &Path::top(0)).unwrap();
        assert_eq!(p0.props_of_char(0).bold, None);
        assert_eq!(p0.props_of_char(1).bold, Some(true));
        let p1 = d.para(StoryRef::Body, &Path::top(1)).unwrap();
        assert_eq!(p1.props_of_char(1).bold, Some(true));
        assert_eq!(p1.props_of_char(2).bold, None);
    }

    #[test]
    fn copy_paste_round_trip() {
        let mut d = doc(&["alpha", "beta", "gamma"]);
        let f = d.copy_range(&Pos::body(0, 2), &Pos::body(2, 3));
        assert_eq!(f.plain_text(), "pha\nbeta\ngam");
        let end = d.insert_fragment(&Pos::body(2, 5), &f).unwrap();
        assert_eq!(d.plain_text(StoryRef::Body), "alpha\nbeta\ngammapha\nbeta\ngam");
        assert_eq!(end, Pos::body(4, 3));
        let one = Fragment::from_text("XY");
        let e = d.insert_fragment(&Pos::body(0, 0), &one).unwrap();
        assert_eq!(e, Pos::body(0, 2));
        assert!(d.plain_text(StoryRef::Body).starts_with("XYalpha"));
    }

    #[test]
    fn tables_in_paths() {
        let mut d = doc(&["before", "after"]);
        d.insert_block(StoryRef::Body, &Path::top(1), Block::Table(Table::new(2, 2, 200.0))).unwrap();
        let paths = d.para_paths(StoryRef::Body);
        assert_eq!(paths.len(), 6);
        assert_eq!(paths[1], Path(vec![1, 0, 0, 0]));
        let cell = Pos { story: StoryRef::Body, path: Path(vec![1, 0, 1, 0]), off: 0 };
        let e = d.insert_text(&cell, "x", &CharProps::default()).unwrap();
        assert_eq!(e.off, 1);
        assert_eq!(d.next_para(StoryRef::Body, &Path(vec![1, 0, 1, 0])), Some(Path(vec![1, 1, 0, 0])));
        assert!(d.plain_text(StoryRef::Body).contains("\tx"));
        // Delete from before the table into a cell: text cleared, table kept (partially covered).
        d.delete_range(&Pos::body(0, 2), &cell).unwrap();
        assert_eq!(d.para(StoryRef::Body, &Path::top(0)).unwrap().text, "be");
        assert!(d.container(StoryRef::Body, &Path(vec![1, 0, 0, 0])).is_some());
        assert_eq!(Path(vec![1, 0, 1, 0]).cell(), Some((Path::top(1), 0, 1)));
    }

    #[test]
    fn bad_positions_error() {
        let mut d = doc(&["é"]);
        assert!(d.insert_text(&Pos::body(0, 1), "x", &CharProps::default()).is_err());
        assert!(d.insert_text(&Pos::body(5, 0), "x", &CharProps::default()).is_err());
        assert!(d.split_paragraph(&Pos::body(0, 99)).is_err());
        let c = d.clamp(&Pos::body(9, 9));
        assert_eq!(c, Pos::body(0, 2));
        assert!(d.delete_range(&Pos { story: StoryRef::Part(3), path: Path::top(0), off: 0 }, &Pos::body(0, 0)).is_err());
    }
}
