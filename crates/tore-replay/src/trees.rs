//! The trees section: display tree samples, each coded against the previous
//! sample of the same subject and channel in the same chunk.
//!
//! The section starts with a directory of the subjects and channels it holds
//! (with the frame of each one's last sample), so a reader can find the
//! latest sample before a tick without decoding every chunk. A delta sample
//! is a list of operations over the previous sample's nodes: keep a run
//! unchanged, skip deleted nodes, edit nodes field by field (numbers as
//! decimal changes), or insert new nodes.

use crate::codec::{In, put_uv};
use crate::error::{Result, corrupt};
use crate::events::{get_value, put_value, value_eq};
use crate::limits::{MAX_TREE_DEPTH, MAX_TREE_NODES, MAX_TREES_PER_TICK};
use crate::model::{Node, TreeSample};
use crate::spawns::{get_gap, put_gap};
use crate::strings::{Interner, StringTable};
use std::collections::HashMap;

const MODE_FULL: u8 = 0;
const MODE_DELTA: u8 = 1;

const OP_SAME: u64 = 0;
const OP_SKIP: u64 = 1;
const OP_EDIT: u64 = 2;
const OP_NEW: u64 = 3;

const F_DEPTH: u8 = 1;
const F_LABEL: u8 = 1 << 1;
const F_VALUE: u8 = 1 << 2;
const F_UNIT: u8 = 1 << 3;
const F_NOTE: u8 = 1 << 4;

/// How far ahead a delta looks for a node whose predecessors were deleted.
const LOOKAHEAD: usize = 8;

fn node_eq(a: &Node, b: &Node) -> bool {
    a.depth == b.depth
        && a.label == b.label
        && value_eq(&a.value, &b.value)
        && a.unit == b.unit
        && a.note == b.note
}

fn same_line(a: &Node, b: &Node) -> bool {
    a.depth == b.depth && a.label == b.label
}

fn put_node(buf: &mut Vec<u8>, strings: &mut Interner, node: &Node) {
    buf.push(node.depth);
    strings.put(buf, &node.label);
    put_value(buf, strings, &node.value, None);
    strings.put(buf, &node.unit);
    strings.put(buf, &node.note);
}

fn get_depth(input: &mut In) -> Result<u8> {
    let depth = input.u8()?;
    if depth >= MAX_TREE_DEPTH {
        return Err(corrupt(format!("a tree node is {depth} levels deep")));
    }
    Ok(depth)
}

fn get_node(input: &mut In, strings: &StringTable) -> Result<Node> {
    Ok(Node {
        depth: get_depth(input)?,
        label: strings.read(input)?,
        value: get_value(input, strings, None)?,
        unit: strings.read(input)?,
        note: strings.read(input)?,
    })
}

fn put_edit(buf: &mut Vec<u8>, strings: &mut Interner, node: &Node, old: &Node) {
    let mut mask = 0;
    if node.depth != old.depth {
        mask |= F_DEPTH;
    }
    if node.label != old.label {
        mask |= F_LABEL;
    }
    if !value_eq(&node.value, &old.value) {
        mask |= F_VALUE;
    }
    if node.unit != old.unit {
        mask |= F_UNIT;
    }
    if node.note != old.note {
        mask |= F_NOTE;
    }
    buf.push(mask);
    if mask & F_DEPTH != 0 {
        buf.push(node.depth);
    }
    if mask & F_LABEL != 0 {
        strings.put(buf, &node.label);
    }
    if mask & F_VALUE != 0 {
        put_value(buf, strings, &node.value, Some(&old.value));
    }
    if mask & F_UNIT != 0 {
        strings.put(buf, &node.unit);
    }
    if mask & F_NOTE != 0 {
        strings.put(buf, &node.note);
    }
}

fn get_edit(input: &mut In, strings: &StringTable, old: &Node) -> Result<Node> {
    let mask = input.u8()?;
    if mask >> 5 != 0 {
        return Err(corrupt("a tree edit has unknown bits"));
    }
    let mut node = old.clone();
    if mask & F_DEPTH != 0 {
        node.depth = get_depth(input)?;
    }
    if mask & F_LABEL != 0 {
        node.label = strings.read(input)?;
    }
    if mask & F_VALUE != 0 {
        node.value = get_value(input, strings, Some(&old.value))?;
    }
    if mask & F_UNIT != 0 {
        node.unit = strings.read(input)?;
    }
    if mask & F_NOTE != 0 {
        node.note = strings.read(input)?;
    }
    Ok(node)
}

enum Op<'a> {
    Same(usize),
    Skip(usize),
    Edit(Vec<(&'a Node, &'a Node)>),
    New(Vec<&'a Node>),
}

/// A short edit script from `old` to `new`, matching lines by depth and label.
fn diff<'a>(old: &'a [Node], new: &'a [Node]) -> Vec<Op<'a>> {
    let mut ops: Vec<Op> = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < new.len() {
        if j < old.len() && node_eq(&new[i], &old[j]) {
            match ops.last_mut() {
                Some(Op::Same(n)) => *n += 1,
                _ => ops.push(Op::Same(1)),
            }
            i += 1;
            j += 1;
        } else if j < old.len() && same_line(&new[i], &old[j]) {
            match ops.last_mut() {
                Some(Op::Edit(nodes)) => nodes.push((&new[i], &old[j])),
                _ => ops.push(Op::Edit(vec![(&new[i], &old[j])])),
            }
            i += 1;
            j += 1;
        } else if let Some(d) =
            (1..=LOOKAHEAD).find(|d| j + d < old.len() && same_line(&new[i], &old[j + d]))
        {
            ops.push(Op::Skip(d));
            j += d;
        } else {
            match ops.last_mut() {
                Some(Op::New(nodes)) => nodes.push(&new[i]),
                _ => ops.push(Op::New(vec![&new[i]])),
            }
            i += 1;
        }
    }
    ops
}

/// Writer state for one chunk's trees section.
#[derive(Default)]
pub(crate) struct TreeCoder {
    previous: HashMap<(u32, String), Vec<Node>>,
    /// Subject, channel, last frame and sample count, in first-seen order.
    directory: Vec<(u32, String, u32, u32)>,
    index: HashMap<(u32, String), usize>,
    entries: u64,
    last_frame: Option<u32>,
    body: Vec<u8>,
}

impl TreeCoder {
    pub fn put(&mut self, frame: u32, trees: &[TreeSample], strings: &mut Interner) {
        if trees.is_empty() {
            return;
        }
        self.entries += 1;
        put_gap(&mut self.body, frame, &mut self.last_frame);
        put_uv(&mut self.body, trees.len() as u64);
        for tree in trees {
            let buf = &mut self.body;
            put_uv(buf, u64::from(tree.subject));
            strings.put(buf, &tree.channel);
            let key = (tree.subject, tree.channel.clone());
            match self.previous.get(&key) {
                Some(old) => {
                    buf.push(MODE_DELTA);
                    put_uv(buf, tree.nodes.len() as u64);
                    for op in diff(old, &tree.nodes) {
                        match op {
                            Op::Same(n) => put_uv(buf, (n as u64) << 2 | OP_SAME),
                            Op::Skip(n) => put_uv(buf, (n as u64) << 2 | OP_SKIP),
                            Op::Edit(nodes) => {
                                put_uv(buf, (nodes.len() as u64) << 2 | OP_EDIT);
                                for (node, old) in nodes {
                                    put_edit(buf, strings, node, old);
                                }
                            }
                            Op::New(nodes) => {
                                put_uv(buf, (nodes.len() as u64) << 2 | OP_NEW);
                                for node in nodes {
                                    put_node(buf, strings, node);
                                }
                            }
                        }
                    }
                }
                None => {
                    buf.push(MODE_FULL);
                    put_uv(buf, tree.nodes.len() as u64);
                    for node in &tree.nodes {
                        put_node(buf, strings, node);
                    }
                }
            }
            match self.index.get(&key) {
                Some(&i) => {
                    self.directory[i].2 = frame;
                    self.directory[i].3 += 1;
                }
                None => {
                    self.index.insert(key.clone(), self.directory.len());
                    self.directory
                        .push((tree.subject, tree.channel.clone(), frame, 1));
                }
            }
            self.previous.insert(key, tree.nodes.clone());
        }
    }

    pub fn section(&self, strings: &mut Interner) -> Option<Vec<u8>> {
        if self.entries == 0 {
            return None;
        }
        let mut out = Vec::with_capacity(self.body.len() + 16 * self.directory.len() + 10);
        put_uv(&mut out, self.directory.len() as u64);
        for (subject, channel, last, count) in &self.directory {
            put_uv(&mut out, u64::from(*subject));
            strings.put(&mut out, channel);
            put_uv(&mut out, u64::from(*last));
            put_uv(&mut out, u64::from(*count));
        }
        put_uv(&mut out, self.entries);
        out.extend_from_slice(&self.body);
        Some(out)
    }

    pub fn len(&self) -> usize {
        self.body.len() + 16 * self.directory.len()
    }
}

/// One directory entry: subject, channel, frame of the last sample, samples.
pub(crate) type DirectoryEntry = (u32, String, u32, u32);

fn get_directory(
    input: &mut In,
    frames: u32,
    strings: &StringTable,
) -> Result<Vec<DirectoryEntry>> {
    let n = input.count(
        frames as usize * MAX_TREES_PER_TICK,
        "tree directory entries",
    )?;
    let mut directory = Vec::with_capacity(n.min(4096));
    for _ in 0..n {
        let subject = input.u32v()?;
        let channel = strings.read(input)?;
        let last = input.u32v()?;
        let count = input.u32v()?;
        if last >= frames {
            return Err(corrupt("a tree directory entry points past the chunk"));
        }
        directory.push((subject, channel, last, count));
    }
    Ok(directory)
}

/// Just the directory, for indexing a file when it opens.
pub(crate) fn get_tree_directory(
    section: &[u8],
    frames: u32,
    strings: &StringTable,
) -> Result<Vec<DirectoryEntry>> {
    get_directory(&mut In::new(section), frames, strings)
}

/// Every sample in the section: `(frame index, sample)`, in order.
pub(crate) fn get_trees(
    section: &[u8],
    frames: u32,
    strings: &StringTable,
) -> Result<Vec<(u32, TreeSample)>> {
    let mut input = In::new(section);
    get_directory(&mut input, frames, strings)?;
    let entries = input.count(frames as usize, "tree entries")?;
    let mut previous: HashMap<(u32, String), Vec<Node>> = HashMap::new();
    let mut out = Vec::new();
    let mut last = None;
    for _ in 0..entries {
        let frame = get_gap(&mut input, &mut last, frames)?;
        let n = input.count(MAX_TREES_PER_TICK, "trees in one frame")?;
        for _ in 0..n {
            let subject = input.u32v()?;
            let channel = strings.read(&mut input)?;
            let key = (subject, channel);
            let mode = input.u8()?;
            let count = input.count(MAX_TREE_NODES, "nodes in one tree")?;
            let mut nodes = Vec::with_capacity(count);
            match mode {
                MODE_FULL => {
                    for _ in 0..count {
                        nodes.push(get_node(&mut input, strings)?);
                    }
                }
                MODE_DELTA => {
                    let old = previous
                        .get(&key)
                        .ok_or_else(|| corrupt("a tree change has no earlier sample"))?;
                    let mut j = 0usize;
                    while nodes.len() < count {
                        let op = input.uv()?;
                        let n = usize::try_from(op >> 2)
                            .ok()
                            .filter(|n| *n >= 1 && *n <= MAX_TREE_NODES)
                            .ok_or_else(|| corrupt("a tree operation has a bad length"))?;
                        let fits_old = j + n <= old.len();
                        let fits_new = nodes.len() + n <= count;
                        match op & 3 {
                            OP_SAME if fits_old && fits_new => {
                                nodes.extend_from_slice(&old[j..j + n]);
                                j += n;
                            }
                            OP_SKIP if fits_old => j += n,
                            OP_EDIT if fits_old && fits_new => {
                                for k in 0..n {
                                    nodes.push(get_edit(&mut input, strings, &old[j + k])?);
                                }
                                j += n;
                            }
                            OP_NEW if fits_new => {
                                for _ in 0..n {
                                    nodes.push(get_node(&mut input, strings)?);
                                }
                            }
                            _ => return Err(corrupt("a tree operation runs past its sample")),
                        }
                    }
                }
                _ => return Err(corrupt("unknown tree encoding")),
            }
            previous.insert(key.clone(), nodes.clone());
            out.push((
                frame,
                TreeSample {
                    subject: key.0,
                    channel: key.1,
                    nodes,
                },
            ));
        }
    }
    if !input.done() {
        return Err(corrupt("the trees section has trailing bytes"));
    }
    Ok(out)
}
