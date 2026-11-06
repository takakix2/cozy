//! Browse モードのツリーモデルと、遅延読み込み＋裏の索引。
//!
//! ⭐ **起動は起点の直下だけ読む**（`#24`）。フォルダは開いたときに中だけ読む。並行して裏のスレッドが
//! 起点の下を全部たどり（`ignore` クレート ＝ ripgrep の walker・`.gitignore`/`.git`/隠しを除外）、
//! 見つけた物は**次の操作のとき**にツリーへ取り込む ——
//! ∴ 冷えたネットワーク上や巨大な起点でも最初の画面は一瞬で出て、`/` の絞り込みは索引ができた所まで
//! （でき上がれば全体まで）届く。
//!
//! 持ち方は平らな arena（`Vec<BrowseNode>`・index 0 = root）。**子は必ず親より後ろの index** に
//! 足される（取り込みは親が居るものだけ・遅延読み込みも親の後ろへ追記）ので、絞り込みの「子→親へ伝播」
//! は index の逆順で一度なめるだけで済む。
//!
//! 描画・操作は「展開済みディレクトリの子だけを深さ優先で並べた可視ノード列」を介して行う。
//! その列はキャッシュしてあり、開閉・絞り込み・ノードの追加があったときだけ数え直す（カーソル移動のたびに
//! 全ノードをなめない）。crossterm/ratatui には依存しない（将来の core/UI 分離方針）。

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// `/` を押したとき、索引の完成をここまで待つ。小さい起点はこの間に終わる ＝ 今までと同じ結果。
/// 大きい起点は途中までで絞り込みを始め、フッターに件数を出す。
const FILTER_WAIT: Duration = Duration::from_millis(300);

/// 起動時にも索引をここまで待つ。📏 `~/dev`（約 6,400 件）の全走査は 62 ms ＝ 普通のリポはこの間に
/// 終わり、最初の画面から今までと同じ（フッターに「索引中」も出ない）。大きい起点だけが遅延読み込みで
/// 先に描く。⚠️ 待たないと、小さい起点でも最初の画面が `indexing 0` のまま、キーを押すまで残る
/// （画面はキーでしか描き直さない）。
const STARTUP_WAIT: Duration = Duration::from_millis(100);

/// 取り込みの小分けの大きさと、1 回の取り込みに使ってよい時間（`#27`）。
/// 📏 1 件あたり約 1.5 µs ＝ 5,000 件で 10 ms 弱。上限が無いと大きい起点で 1 回に数十万件を抱え、
/// その間キーも描き直しも止まった（`/usr` で約 1 秒・`/Applications` 21 万件で 328 ms）。
/// ⚠️ 件数だけで区切ると、200 ms おきの tick では索引が終わるまでが延びる（`/Applications` で 0.94 s →
/// 2.57 s）—— ∴ **時間で区切る**: 小分けを予算の間くり返す。
const ABSORB_CHUNK: usize = 5_000;
const ABSORB_BUDGET: Duration = Duration::from_millis(50);

/// ツリー上の 1 エントリ（ファイル or ディレクトリ）。
#[derive(Debug, Clone)]
pub struct BrowseNode {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    /// root からの相対深さ（root = 0）。描画のインデントに使う。
    pub depth: usize,
    pub parent: Option<usize>,
    /// ディレクトリ先・名前順にソート済みの子 index。
    pub children: Vec<usize>,
    /// 小文字の名前（並べ替えと絞り込みの照合用・毎回作らない）。
    name_lower: String,
    /// 中身を読んだか（ディレクトリだけ意味を持つ）。索引が完成したら全部読んだ扱い。
    loaded: bool,
}

/// 裏の索引作り。見つけた物は**追記だけ**の一覧に貯め、各ツリーは自分が読んだ位置を持つ
/// （clone されても取り合わない）。
#[derive(Debug, Default)]
struct IndexJob {
    found: Mutex<Vec<(PathBuf, bool)>>,
    done: AtomicBool,
    cancel: AtomicBool,
}

/// ツリーが手放したら索引作りを止める（索引のスレッドは `IndexJob` を握り続けるので、
/// 止める合図はツリー側の持ち物にする）。
#[derive(Debug)]
struct IndexHandle(Arc<IndexJob>);

impl Drop for IndexHandle {
    fn drop(&mut self) {
        self.0.cancel.store(true, Ordering::Relaxed);
    }
}

#[derive(Debug, Clone)]
struct VisibleCache {
    filter: String,
    generation: u64,
    nodes: Vec<usize>,
}

/// ツリー全体と現在の表示・選択状態。
#[derive(Debug, Clone)]
pub struct BrowseTree {
    pub root: PathBuf,
    pub nodes: Vec<BrowseNode>,
    /// カーソル位置のノード index（常に可視ノードのいずれか）。
    pub selected: usize,
    /// `/` 絞り込み文字列（空＝絞り込みなし）。書き換えは [`Self::begin_filter`] ほかを通す。
    pub filter: String,
    /// `/` 絞り込み入力を受け付けている最中か。
    pub filtering: bool,
    /// 展開中ディレクトリの index（root は常に展開）。
    expanded: HashSet<usize>,
    index_of: HashMap<PathBuf, usize>,
    /// 裏の索引作り（完成して取り込み終えたら `None`）。
    index: Option<Arc<IndexHandle>>,
    /// 索引の一覧のうち、ここまで取り込んだ。
    absorbed: usize,
    /// 取り込んだが、親がまだツリーに居ない物。
    pending: Vec<(PathBuf, bool)>,
    /// 開閉・ノードの追加のたびに進む（可視列のキャッシュの鍵）。
    generation: u64,
    visible: RefCell<Option<VisibleCache>>,
}

impl BrowseTree {
    /// `root` の直下を読み、裏で起点の下全体をたどり始めて、[`STARTUP_WAIT`] まで索引を待つ。
    pub fn build(root: &Path) -> Self {
        Self::build_waiting(root, STARTUP_WAIT)
    }

    fn build_waiting(root: &Path, wait: Duration) -> Self {
        let mut tree = Self::empty(root);
        tree.ensure_loaded(0);
        tree.index = Some(Arc::new(IndexHandle(spawn_index(root))));
        if !wait.is_zero() {
            tree.wait_for_index(wait);
        }
        tree
    }

    fn empty(root: &Path) -> Self {
        let root = root.to_path_buf();
        let name = root
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| root.to_string_lossy().to_string());
        let mut index_of = HashMap::new();
        index_of.insert(root.clone(), 0);
        let mut expanded = HashSet::new();
        expanded.insert(0); // root は常に展開
        BrowseTree {
            nodes: vec![BrowseNode {
                path: root.clone(),
                name_lower: name.to_lowercase(),
                name,
                is_dir: true,
                depth: 0,
                parent: None,
                children: Vec::new(),
                loaded: false,
            }],
            root,
            selected: 0,
            filter: String::new(),
            filtering: false,
            expanded,
            index_of,
            index: None,
            absorbed: 0,
            pending: Vec::new(),
            generation: 0,
            visible: RefCell::new(None),
        }
    }

    /// 索引作りの途中なら、見つかった件数（フッターに出す）。完成していれば `None`。
    pub fn indexing_progress(&self) -> Option<usize> {
        let handle = self.index.as_ref()?;
        let found = handle.0.found.lock().map(|f| f.len()).unwrap_or(0);
        Some(found)
    }

    /// 索引の完成を `timeout` まで待ち、取り込む。完成したら `true`。
    pub fn wait_for_index(&mut self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while let Some(handle) = &self.index {
            if handle.0.done.load(Ordering::Acquire) || Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        // 残りの予算の間は取り込みをくり返す（1 回分だけだと、走査済みなのに一部しか探さない）。
        let left = deadline.saturating_duration_since(Instant::now());
        self.absorb_within(left.max(ABSORB_BUDGET));
        self.index.is_none()
    }

    /// 裏の索引がまだ途中か —— `event_loop` はこの間だけ短い間隔で起きて描き直す（`#25`）。
    pub fn is_indexing(&self) -> bool {
        self.index.is_some()
    }

    /// キーが来ない間に呼ぶ: 見つかった物を取り込む。索引が途中だった（＝件数か中身が
    /// 動いたかもしれない）なら `true` —— 完成を取り込んだ回も `true`（`indexing` を消す描き直しが要る）。
    pub fn tick(&mut self) -> bool {
        if self.index.is_none() {
            return false;
        }
        self.absorb();
        true
    }

    /// テスト用: 裏の索引が終わらないままのツリー（直下も読まない）。
    #[cfg(test)]
    pub(crate) fn stalled(root: &Path) -> Self {
        let mut tree = Self::empty(root);
        tree.index = Some(Arc::new(IndexHandle(Arc::new(IndexJob::default()))));
        tree
    }

    /// テスト用: [`Self::stalled`] の索引に「完成」の印を立てる。
    #[cfg(test)]
    pub(crate) fn finish_stalled_index(&self) {
        if let Some(handle) = &self.index {
            handle.0.done.store(true, Ordering::Release);
        }
    }

    pub fn is_expanded(&self, idx: usize) -> bool {
        self.expanded.contains(&idx)
    }

    /// ディレクトリを開く／閉じる（開くときは中を読む）。
    pub fn set_expanded(&mut self, idx: usize, open: bool) {
        if open {
            self.ensure_loaded(idx);
            self.expanded.insert(idx);
        } else if idx != 0 {
            self.expanded.remove(&idx);
        }
        self.generation += 1;
    }

    fn is_filtering(&self) -> bool {
        !self.filter.is_empty()
    }

    /// 画面に出すノード index を上から順に返す。
    /// 通常は展開済みディレクトリの子を深さ優先で。絞り込み中はマッチノードとその祖先のみ
    /// （ディレクトリは自動展開扱い）。開閉・絞り込み・ノードの追加が無ければ前回の列を返す。
    pub fn visible_nodes(&self) -> Vec<usize> {
        if let Some(cache) = self.visible.borrow().as_ref()
            && cache.generation == self.generation
            && cache.filter == self.filter
        {
            return cache.nodes.clone();
        }
        let nodes = self.compute_visible();
        *self.visible.borrow_mut() = Some(VisibleCache {
            filter: self.filter.clone(),
            generation: self.generation,
            nodes: nodes.clone(),
        });
        nodes
    }

    fn compute_visible(&self) -> Vec<usize> {
        let keep = if self.is_filtering() {
            let needle = self.filter.to_lowercase();
            let mut keep = vec![false; self.nodes.len()];
            // 子は親より後ろの index → 逆順で子→親に伝播。
            for i in (0..self.nodes.len()).rev() {
                let node = &self.nodes[i];
                let child_kept = node.children.iter().any(|&c| keep[c]);
                keep[i] = child_kept || node.name_lower.contains(&needle);
            }
            Some(keep)
        } else {
            None
        };

        let mut out = Vec::new();
        self.collect_visible(0, &keep, &mut out);
        out
    }

    fn collect_visible(&self, idx: usize, keep: &Option<Vec<bool>>, out: &mut Vec<usize>) {
        if let Some(keep) = keep
            && !keep[idx]
        {
            return;
        }
        out.push(idx);
        // 絞り込み中は全ディレクトリを展開扱い。
        let expanded = keep.is_some() || self.expanded.contains(&idx);
        if expanded {
            for &child in &self.nodes[idx].children {
                self.collect_visible(child, keep, out);
            }
        }
    }

    /// 可視列上でカーソルを 1 つ下へ。
    pub fn move_down(&mut self) {
        self.absorb();
        let vis = self.visible_nodes();
        match vis.iter().position(|&i| i == self.selected) {
            Some(pos) if pos + 1 < vis.len() => self.selected = vis[pos + 1],
            None => {
                if let Some(&first) = vis.first() {
                    self.selected = first;
                }
            }
            _ => {}
        }
    }

    /// 可視列上でカーソルを 1 つ上へ。
    pub fn move_up(&mut self) {
        self.absorb();
        let vis = self.visible_nodes();
        match vis.iter().position(|&i| i == self.selected) {
            Some(pos) if pos > 0 => self.selected = vis[pos - 1],
            None => {
                if let Some(&first) = vis.first() {
                    self.selected = first;
                }
            }
            _ => {}
        }
    }

    pub fn goto_top(&mut self) {
        self.absorb();
        if let Some(&first) = self.visible_nodes().first() {
            self.selected = first;
        }
    }

    pub fn goto_bottom(&mut self) {
        self.absorb();
        if let Some(&last) = self.visible_nodes().last() {
            self.selected = last;
        }
    }

    /// `l`/Enter/→: ディレクトリなら展開トグル（None）、ファイルなら開く対象 path を返す。
    pub fn expand_or_open(&mut self) -> Option<PathBuf> {
        self.absorb();
        let idx = self.selected;
        if self.nodes[idx].is_dir {
            let open = !self.expanded.contains(&idx);
            self.set_expanded(idx, open);
            None
        } else {
            Some(self.nodes[idx].path.clone())
        }
    }

    /// `h`/←: 展開中ディレクトリは畳む、それ以外は親ノードへ移動。
    pub fn collapse_or_parent(&mut self) {
        self.absorb();
        let idx = self.selected;
        if idx != 0 && self.nodes[idx].is_dir && self.expanded.contains(&idx) {
            self.set_expanded(idx, false);
        } else if let Some(parent) = self.nodes[idx].parent {
            self.selected = parent;
        }
    }

    /// 指定 path のノードを選択し、その祖先をすべて展開する（現ファイルのプリセレクト用）。
    /// まだ読んでいない階層は、通り道のフォルダだけその場で読む。見つからなければ何もしない。
    pub fn select_path(&mut self, path: &Path) {
        self.absorb();
        let Ok(rel) = path.strip_prefix(&self.root) else {
            return;
        };
        let mut chain = Vec::new();
        let mut cur = 0;
        let mut cur_path = self.root.clone();
        for comp in rel.components() {
            self.ensure_loaded(cur);
            cur_path.push(comp);
            let Some(&child) = self.index_of.get(&cur_path) else {
                return;
            };
            chain.push(cur);
            cur = child;
        }
        for dir in chain {
            self.expanded.insert(dir);
        }
        self.generation += 1;
        self.selected = cur;
    }

    /// `/`: 絞り込みの入力を始める。索引が途中なら [`FILTER_WAIT`] まで待つ。
    pub fn begin_filter(&mut self) {
        self.filtering = true;
        self.filter.clear();
        self.wait_for_index(FILTER_WAIT);
    }

    /// 絞り込みをやめる（入力も結果も捨てる）。
    pub fn clear_filter(&mut self) {
        self.filtering = false;
        self.filter.clear();
        self.reclamp_selection();
    }

    /// 絞り込み文字を 1 文字追加。
    pub fn push_filter_char(&mut self, c: char) {
        self.absorb();
        self.filter.push(c);
        self.reclamp_selection();
    }

    /// 絞り込み文字を 1 文字削除。
    pub fn pop_filter_char(&mut self) {
        self.absorb();
        self.filter.pop();
        self.reclamp_selection();
    }

    /// 選択が現在の可視列から外れていたら先頭に寄せ直す。
    fn reclamp_selection(&mut self) {
        let vis = self.visible_nodes();
        if !vis.contains(&self.selected) {
            self.selected = vis.first().copied().unwrap_or(0);
        }
    }

    /// ディレクトリの中を（まだなら）読んで子として足す。
    fn ensure_loaded(&mut self, idx: usize) {
        if !self.nodes[idx].is_dir || self.nodes[idx].loaded {
            return;
        }
        let dir = self.nodes[idx].path.clone();
        let mut touched = HashSet::new();
        for (path, is_dir) in read_dir_filtered(&dir) {
            self.insert(path, is_dir, idx, &mut touched);
        }
        self.nodes[idx].loaded = true;
        self.sort_children(touched);
    }

    /// 裏の索引で見つかった物を取り込む（1 回に [`ABSORB_LIMIT`] 件まで）。完成していて全部取り込めたら、
    /// 索引を手放す。
    ///
    /// ⭐ **並べ直さない**（`#27`）—— `ignore` の walker はフォルダを読んでから中身を出すので、一覧の時点で
    /// 親は子より前に居る。📏 4 つの起点・約 46 万件で、親が見つからず待たされた物は 0 件だった。
    /// それでも親の居ない物が来たら `pending` に残し、次の回で試し直す（並べ直しは 1 回の取り込みの約半分を
    /// 食っていた）。
    fn absorb(&mut self) {
        self.absorb_within(ABSORB_BUDGET);
    }

    /// 小分けの取り込みを `budget` の間くり返す（少なくとも 1 回はやる）。追いついたらやめる。
    fn absorb_within(&mut self, budget: Duration) {
        let deadline = Instant::now() + budget;
        while !self.absorb_chunk() && Instant::now() < deadline {}
    }

    /// 小分けを 1 回取り込む。追いついた（または索引が無い）なら `true`。
    fn absorb_chunk(&mut self) -> bool {
        let Some(handle) = &self.index else {
            return true;
        };
        // ⚠️ 完成の印を**先に**読む —— 読んだ後に足された物を「完成」と取り違えないように。
        let done = handle.0.done.load(Ordering::Acquire);
        let (fresh, found_total): (Vec<(PathBuf, bool)>, usize) = match handle.0.found.lock() {
            Ok(found) => {
                let end = found.len().min(self.absorbed + ABSORB_CHUNK);
                (found[self.absorbed..end].to_vec(), found.len())
            }
            Err(_) => (Vec::new(), self.absorbed),
        };
        self.absorbed += fresh.len();
        let caught_up = self.absorbed >= found_total;
        if fresh.is_empty() && self.pending.is_empty() && !done {
            return true;
        }
        let mut touched = HashSet::new();
        let mut waiting = Vec::new();
        let retry = std::mem::take(&mut self.pending);
        self.place(retry.into_iter().chain(fresh), &mut waiting, &mut touched);
        let finished = done && caught_up;
        if finished {
            // 最後の回: 進まなくなるまで試し直し、それでも親の居ない物は捨てる。
            while !waiting.is_empty() {
                let before = waiting.len();
                let again = std::mem::take(&mut waiting);
                self.place(again.into_iter(), &mut waiting, &mut touched);
                if waiting.len() == before {
                    waiting.clear();
                }
            }
        }
        self.pending = waiting;
        self.sort_children(touched);
        if finished {
            // 起点の下は全部たどった ＝ どのフォルダも読み終えた扱い（開いても読み直さない）。
            for node in &mut self.nodes {
                node.loaded = true;
            }
            self.index = None;
        }
        caught_up
    }

    /// 親の居る物を足し、居ない物は `waiting` へ。
    fn place(
        &mut self,
        entries: impl Iterator<Item = (PathBuf, bool)>,
        waiting: &mut Vec<(PathBuf, bool)>,
        touched: &mut HashSet<usize>,
    ) {
        for (path, is_dir) in entries {
            match path.parent().and_then(|p| self.index_of.get(p)).copied() {
                Some(parent) => self.insert(path, is_dir, parent, touched),
                None => waiting.push((path, is_dir)),
            }
        }
    }

    /// 1 件足す（既に在れば何もしない）。`parent` は呼び手が引いた親の index。
    /// 子の並べ替えは呼び手がまとめてやる。⭐ 表を引くのは entry の 1 回だけ（`#27`）。
    fn insert(&mut self, path: PathBuf, is_dir: bool, parent: usize, touched: &mut HashSet<usize>) {
        let idx = self.nodes.len();
        match self.index_of.entry(path.clone()) {
            std::collections::hash_map::Entry::Occupied(_) => return,
            std::collections::hash_map::Entry::Vacant(slot) => {
                slot.insert(idx);
            }
        }
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        self.nodes.push(BrowseNode {
            path,
            name_lower: name.to_lowercase(),
            name,
            is_dir,
            depth: self.nodes[parent].depth + 1,
            parent: Some(parent),
            children: Vec::new(),
            loaded: false,
        });
        self.nodes[parent].children.push(idx);
        touched.insert(parent);
    }

    /// 子が増えたディレクトリだけ、ディレクトリ先・名前昇順に並べ直す。
    fn sort_children(&mut self, touched: HashSet<usize>) {
        if touched.is_empty() {
            return;
        }
        for dir in touched {
            let mut children = std::mem::take(&mut self.nodes[dir].children);
            children.sort_by(|&a, &b| {
                let (a, b) = (&self.nodes[a], &self.nodes[b]);
                (!a.is_dir, &a.name_lower).cmp(&(!b.is_dir, &b.name_lower))
            });
            self.nodes[dir].children = children;
        }
        self.generation += 1;
    }
}

/// 1 つのフォルダの直下を、全体の走査と同じ規則（`.gitignore`/`.git`/隠し）で読む。
fn read_dir_filtered(dir: &Path) -> Vec<(PathBuf, bool)> {
    ignore::WalkBuilder::new(dir)
        .standard_filters(true)
        .max_depth(Some(1))
        .build()
        .flatten()
        .filter(|entry| entry.depth() == 1)
        .map(|entry| {
            let is_dir = entry.file_type().is_some_and(|t| t.is_dir());
            (entry.into_path(), is_dir)
        })
        .collect()
}

/// 裏のスレッドで `root` の下を全部たどる（`ignore` の並列ウォーカー）。
fn spawn_index(root: &Path) -> Arc<IndexJob> {
    let job = Arc::new(IndexJob::default());
    let worker = Arc::clone(&job);
    let root = root.to_path_buf();
    std::thread::spawn(move || {
        let threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        ignore::WalkBuilder::new(&root)
            .standard_filters(true)
            .threads(threads)
            .build_parallel()
            .run(|| {
                let job = Arc::clone(&worker);
                Box::new(move |result| {
                    if job.cancel.load(Ordering::Relaxed) {
                        return ignore::WalkState::Quit;
                    }
                    if let Ok(entry) = result
                        && entry.depth() > 0
                    {
                        let is_dir = entry.file_type().is_some_and(|t| t.is_dir());
                        if let Ok(mut found) = job.found.lock() {
                            found.push((entry.into_path(), is_dir));
                        }
                    }
                    ignore::WalkState::Continue
                })
            });
        worker.done.store(true, Ordering::Release);
    });
    job
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn tick_reports_work_until_the_index_is_absorbed() {
        let mut tree = BrowseTree::stalled(Path::new("/nonexistent-cozy-root"));
        assert!(tree.is_indexing());
        assert!(tree.tick(), "途中なら描き直す価値がある");
        assert!(tree.is_indexing(), "完成の印が無いうちは手放さない");

        tree.finish_stalled_index();
        assert!(
            tree.tick(),
            "完成を取り込んだ回も描き直す（`indexing` を消すため）"
        );
        assert!(!tree.is_indexing());
        assert!(!tree.tick(), "取り込み終えたら何もしない");
    }

    /// 索引の完成を待つ上限（テストの木は小さいので、普段は数 ms で終わる）。
    const WAIT: Duration = Duration::from_secs(10);

    /// 一時ディレクトリツリーを作る簡易ヘルパ。返す `TempDir` を握っている間だけ在り、
    /// drop で消える（呼び手は `let (_tmp, base) = …` で関数の終わりまで持つ）。
    fn scratch_tree(name: &str) -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::Builder::new()
            .prefix(&format!("cozy_browse_test_{name}_"))
            .tempdir()
            .unwrap();
        let base = tmp.path().to_path_buf();
        fs::create_dir_all(base.join("src")).unwrap();
        fs::create_dir_all(base.join("target/debug")).unwrap();
        fs::create_dir_all(base.join(".git")).unwrap();
        fs::write(base.join("README.md"), "readme").unwrap();
        fs::write(base.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(base.join("src/lib.rs"), "// lib").unwrap();
        fs::write(base.join("target/debug/artifact"), "bin").unwrap();
        fs::write(base.join(".git/config"), "[core]").unwrap();
        (tmp, base)
    }

    fn names(tree: &BrowseTree) -> Vec<&str> {
        tree.visible_nodes()
            .iter()
            .map(|&i| tree.nodes[i].name.as_str())
            .collect()
    }

    fn count(tree: &BrowseTree, name: &str) -> usize {
        tree.nodes.iter().filter(|n| n.name == name).count()
    }

    #[test]
    fn build_collects_files_and_dirs() {
        let (_tmp, base) = scratch_tree("build_collects");
        let mut tree = BrowseTree::build(&base);
        assert!(tree.wait_for_index(WAIT), "索引が完成する");
        assert!(tree.nodes.iter().any(|n| n.name == "README.md"));
        assert!(tree.nodes.iter().any(|n| n.name == "main.rs"));
        assert!(tree.nodes.iter().any(|n| n.name == "src" && n.is_dir));
    }

    #[test]
    fn git_and_target_are_excluded() {
        let (_tmp, base) = scratch_tree("git_target");
        let mut tree = BrowseTree::build_waiting(&base, Duration::ZERO);
        // 直下を読んだ時点（遅延読み込み）でも、索引の後でも、.git は入らない。
        assert!(!tree.nodes.iter().any(|n| n.name == ".git"));
        tree.wait_for_index(WAIT);
        assert!(
            !tree
                .nodes
                .iter()
                .any(|n| n.path.components().any(|c| c.as_os_str() == ".git"))
        );
    }

    /// `#24`: 起動は直下だけ —— 索引を待たずに返った時点で、深い物はまだ読んでいない。
    /// ⭐ 待たない build は取り込みをしないので、裏の索引が先に終わっていてもこの形は崩れない。
    #[test]
    fn build_reads_only_the_top_level() {
        let (_tmp, base) = scratch_tree("top_level");
        let tree = BrowseTree::build_waiting(&base, Duration::ZERO);
        assert_eq!(count(&tree, "src"), 1);
        assert_eq!(count(&tree, "README.md"), 1);
        assert_eq!(count(&tree, "main.rs"), 0, "src の中はまだ読んでいない");
    }

    /// 小さい起点は起動の予算の間に索引が終わる ＝ フッターに「索引中」が残らない。
    #[test]
    fn a_small_root_is_fully_indexed_on_startup() {
        let (_tmp, base) = scratch_tree("startup_wait");
        let tree = BrowseTree::build(&base);
        assert_eq!(tree.indexing_progress(), None);
        assert_eq!(count(&tree, "main.rs"), 1);
    }

    #[test]
    fn opening_a_folder_reads_it() {
        let (_tmp, base) = scratch_tree("expand_collapse");
        let mut tree = BrowseTree::build_waiting(&base, Duration::ZERO);
        let src_idx = tree.nodes.iter().position(|n| n.name == "src").unwrap();
        // 初期は root のみ展開 → src は見えるが main.rs は見えない。
        assert!(tree.visible_nodes().contains(&src_idx));
        assert!(!names(&tree).contains(&"main.rs"));
        // src を開くと中を読み、main.rs が見える。閉じると消える。
        tree.set_expanded(src_idx, true);
        assert!(names(&tree).contains(&"main.rs"));
        tree.set_expanded(src_idx, false);
        assert!(!names(&tree).contains(&"main.rs"));
    }

    /// 遅延読み込みで先に入った物を、索引が二重に足さない。並びはディレクトリ先・名前順のまま。
    #[test]
    fn the_index_does_not_duplicate_what_was_read_lazily() {
        let (_tmp, base) = scratch_tree("no_dup");
        let mut tree = BrowseTree::build_waiting(&base, Duration::ZERO);
        tree.selected = tree.nodes.iter().position(|n| n.name == "src").unwrap();
        tree.expand_or_open();
        assert!(tree.wait_for_index(WAIT));
        assert_eq!(count(&tree, "main.rs"), 1);
        assert_eq!(count(&tree, "src"), 1);
        let src = &tree.nodes[tree.nodes.iter().position(|n| n.name == "src").unwrap()];
        let kids: Vec<&str> = src
            .children
            .iter()
            .map(|&c| tree.nodes[c].name.as_str())
            .collect();
        assert_eq!(kids, vec!["lib.rs", "main.rs"]);
        // root の子もディレクトリ先（索引で来た target が src より前・README.md は後ろ）。
        let root_kids: Vec<&str> = tree.nodes[0]
            .children
            .iter()
            .map(|&c| tree.nodes[c].name.as_str())
            .collect();
        assert_eq!(root_kids, vec!["src", "target", "README.md"]);
    }

    /// `/` は小さい起点なら索引の完成を待つ ＝ まだ開いていない深いフォルダまで届く（今までと同じ）。
    #[test]
    fn filter_reaches_folders_that_were_never_opened() {
        let (_tmp, base) = scratch_tree("filter");
        let mut tree = BrowseTree::build_waiting(&base, Duration::ZERO);
        tree.begin_filter();
        for c in "main".chars() {
            tree.push_filter_char(c);
        }
        let vis = names(&tree);
        assert!(vis.contains(&"main.rs")); // マッチ
        assert!(vis.contains(&"src")); // 祖先
        assert!(!vis.contains(&"README.md")); // 非マッチ
        tree.clear_filter();
        assert!(!tree.filtering);
        assert!(
            !names(&tree).contains(&"main.rs"),
            "絞り込みをやめたら畳んだ形に戻る"
        );
    }

    /// 起動時のプリセレクトは、通り道のフォルダだけその場で読む（索引を待たない）。
    #[test]
    fn select_path_reads_the_way_down() {
        let (_tmp, base) = scratch_tree("select_deep");
        fs::create_dir_all(base.join("a/b/c")).unwrap();
        fs::write(base.join("a/b/c/deep.txt"), "x").unwrap();
        let mut tree = BrowseTree::build_waiting(&base, Duration::ZERO);
        tree.select_path(&base.join("a/b/c/deep.txt"));
        assert_eq!(tree.nodes[tree.selected].name, "deep.txt");
        let vis = names(&tree);
        for name in ["a", "b", "c", "deep.txt"] {
            assert!(vis.contains(&name), "{name} が見える: {vis:?}");
        }
        // 無い物を指したら何も変えない。
        let before = tree.selected;
        tree.select_path(&base.join("a/nope.txt"));
        assert_eq!(tree.selected, before);
    }

    /// 可視列のキャッシュは、開閉・絞り込みで作り直される（古い列を返さない）。
    #[test]
    fn the_visible_list_follows_every_change() {
        let (_tmp, base) = scratch_tree("cache");
        let mut tree = BrowseTree::build(&base);
        tree.wait_for_index(WAIT);
        let collapsed = tree.visible_nodes();
        let src_idx = tree.nodes.iter().position(|n| n.name == "src").unwrap();
        tree.set_expanded(src_idx, true);
        let opened = tree.visible_nodes();
        assert_eq!(
            opened.len(),
            collapsed.len() + 2,
            "lib.rs と main.rs が増える"
        );
        tree.filter = "lib".to_string(); // 直接書いても値で比べるので古い列は返らない
        assert_eq!(
            names(&tree),
            vec![tree.nodes[0].name.as_str(), "src", "lib.rs"]
        );
        tree.filter.clear();
        assert_eq!(tree.visible_nodes(), opened);
    }

    /// ツリーを手放したら、裏の索引作りに止めの合図が届く。
    #[test]
    fn dropping_the_tree_stops_the_index() {
        let (_tmp, base) = scratch_tree("drop");
        let tree = BrowseTree::build_waiting(&base, Duration::ZERO);
        let job = Arc::clone(&tree.index.as_ref().unwrap().0);
        let copy = tree.clone();
        drop(tree);
        assert!(
            !job.cancel.load(Ordering::Relaxed),
            "clone が残っている間は止めない"
        );
        drop(copy);
        assert!(job.cancel.load(Ordering::Relaxed));
    }

    /// 索引の一覧に、止まった索引の中身として直接積む（ファイルシステムは要らない）。
    fn feed(tree: &BrowseTree, entries: impl IntoIterator<Item = (PathBuf, bool)>) {
        let handle = tree.index.as_ref().expect("索引が要る");
        handle.0.found.lock().unwrap().extend(entries);
    }

    /// `#27`: 小分け 1 回は上限まで。完成の印は「全部取り込んだとき」だけ。
    #[test]
    fn one_chunk_takes_at_most_the_limit() {
        let root = Path::new("/nonexistent-cozy-root");
        let mut tree = BrowseTree::stalled(root);
        let total = ABSORB_CHUNK * 2 + 7;
        feed(
            &tree,
            (0..total).map(|i| (root.join(format!("f{i}")), false)),
        );
        tree.finish_stalled_index();
        assert!(!tree.absorb_chunk(), "まだ追いついていない");
        assert_eq!(tree.nodes.len(), 1 + ABSORB_CHUNK, "1 回目は上限まで");
        assert!(tree.is_indexing(), "取り込み切るまで完成と言わない");
        tree.absorb_chunk();
        assert_eq!(tree.nodes.len(), 1 + ABSORB_CHUNK * 2);
        assert!(tree.is_indexing());
        assert!(tree.absorb_chunk());
        assert_eq!(tree.nodes.len(), 1 + total);
        assert!(!tree.is_indexing(), "全部取り込んだら完成");
        assert_eq!(tree.nodes[0].children.len(), total);
    }

    /// 予算の間は小分けをくり返す —— 走査が終わっていれば `/` の待ちで全部取り込む（1 回分で止まらない）。
    #[test]
    fn waiting_for_the_index_absorbs_every_chunk() {
        let root = Path::new("/nonexistent-cozy-root");
        let mut tree = BrowseTree::stalled(root);
        let total = ABSORB_CHUNK * 3 + 1;
        feed(
            &tree,
            (0..total).map(|i| (root.join(format!("f{i}")), false)),
        );
        tree.finish_stalled_index();
        assert!(tree.wait_for_index(Duration::from_secs(10)));
        assert_eq!(tree.nodes.len(), 1 + total);
    }

    /// 並べ直さないので、子が親より先に来たら待たせて、親が来た後で足す。
    #[test]
    fn a_child_that_arrives_before_its_parent_waits_for_it() {
        let root = Path::new("/nonexistent-cozy-root");
        let mut tree = BrowseTree::stalled(root);
        feed(
            &tree,
            [(root.join("a/b/c.txt"), false), (root.join("a/b"), true)],
        );
        tree.tick();
        assert_eq!(count(&tree, "c.txt"), 0, "親（a）がまだ居ない");
        feed(&tree, [(root.join("a"), true)]);
        tree.finish_stalled_index();
        tree.tick();
        for name in ["a", "b", "c.txt"] {
            assert_eq!(count(&tree, name), 1, "{name}");
        }
        let c = tree.nodes.iter().position(|n| n.name == "c.txt").unwrap();
        assert_eq!(tree.nodes[c].depth, 3);
        assert!(!tree.is_indexing());
    }

    /// 同じ物が 2 度届いても 1 つだけ（遅延読み込みと索引の両方から来る形）。
    #[test]
    fn the_same_entry_twice_is_one_node() {
        let root = Path::new("/nonexistent-cozy-root");
        let mut tree = BrowseTree::stalled(root);
        feed(&tree, [(root.join("x"), false), (root.join("x"), false)]);
        tree.finish_stalled_index();
        tree.tick();
        assert_eq!(count(&tree, "x"), 1);
        assert_eq!(tree.nodes[0].children.len(), 1);
    }

    #[test]
    fn move_down_up_clamps_at_bounds() {
        let (_tmp, base) = scratch_tree("move_bounds");
        let mut tree = BrowseTree::build(&base);
        tree.goto_top();
        let top = tree.selected;
        tree.move_up(); // 先頭で上 → 変化なし
        assert_eq!(tree.selected, top);
        tree.goto_bottom();
        let bottom = tree.selected;
        tree.move_down(); // 末尾で下 → 変化なし
        assert_eq!(tree.selected, bottom);
    }
}
