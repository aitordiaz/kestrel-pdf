# PLAN-0017: Individual Search Occurrences, Active Context Snippets, and Document Counter Badge

- **Target Version**: v0.3.1
- **Author**: Antigravity (AI Pair Programmer)
- **Status**: In Progress
- **Date**: 2026-10-13

---

## 1. Objective & Scope

Users identified three key limitations in the existing search user experience:
1. **Aggregated Page List in Sidebar**:
   Instead of displaying individual matches, the sidebar collapsed search results to one entry per page. Multiple occurrences on the same page were hidden behind a single entry.
2. **Missing Active Occurrence Context**:
   Users need to see each individual occurrence with surrounding text (context snippet before and after), displayed in a scrollable list where clicking any entry navigates to that exact occurrence, highlighting the active item in a distinctive color.
3. **Missing `X / N` Counter & Global Arrow Navigation in Search Panel**:
   The toolbar search bar lacked a global counter showing `X / N` (where `X` is the active match index and `N` is the total count of matches across the entire document), along with always-available previous (`◀`) and next (`▶`) buttons to navigate between all occurrences.
4. **Canvas Active Match Highlight Distinction**:
   On the PDF canvas, all matches on a page were colored in the same active color. Only the specific active match should receive the active highlight color (`SEARCH_HIGHLIGHT_ACTIVE`), while other matches on the same page and document display in the standard highlight color (`SEARCH_HIGHLIGHT_REGULAR`).

---

## 2. Technical Specification

### 2.1 Core Search Model (`kestrel-core`)
- Extend `SearchResult`:
  ```rust
  #[derive(Debug, Clone, PartialEq)]
  pub struct SearchResult {
      pub page_index: u16,
      pub snippet: String,
      pub match_count: usize,
      pub match_index_in_page: usize,
      pub global_match_index: usize,
  }
  ```
- Update `DocumentSession::search_text(&self, query: &str) -> Vec<SearchResult>`:
  - Iterate through all pages and all match occurrences on each page.
  - Generate a safe context snippet with UTF-8 character boundary validation:
    ```rust
    fn safe_snippet(text: &str, byte_idx: usize, query_len: usize) -> String
    ```
  - Return an entry for every individual occurrence, assigning `match_index_in_page` and sequential `global_match_index`.

### 2.2 Sidebar Occurrences List (`kestrel-app`)
- In `SidebarTab::SearchResults`:
  - Header: `Coincidencias (X / N):` (e.g., `Coincidencias (1 / 4):`).
  - Render an individual card for each occurrence:
    - Label: `#X · Pág. Y`
    - Context snippet: displaying text before and after the query with word wrapping.
    - Active card styling: highlighted border (`ACCENT_OCHRE`), distinct background fill, and `● Activa` indicator when selected.
    - Click handler: clicking anywhere on the card invokes `navigate_to_search_result(idx)`.

### 2.3 Toolbar Search Panel & `X / N` Counter (`kestrel-app`)
- In the toolbar search group:
  - Text input with search query.
  - Counter badge: displaying `X / N` (e.g. `1 / 4` or `0 / 0` when no results).
  - Always-present navigation buttons: `◀` (previous) and `▶` (next), enabled when matches exist, with tooltips and shortcuts (Enter / Shift+Enter).

### 2.4 Canvas Highlight & Viewport Scrolling (`kestrel-app`)
- In `CentralPanel`:
  - Track `page_match_counter: usize` per page as text runs are rendered.
  - Match against `sel_res.page_index == page_idx && sel_res.match_index_in_page == page_match_counter`.
  - Color the exact active match with `SEARCH_HIGHLIGHT_ACTIVE` (warm amber), and all other matches with `SEARCH_HIGHLIGHT_REGULAR` (sunny yellow).
  - Programmatically scroll viewport to `hl_rect` when `is_active_result` and `self.scroll_to_search_match` is true.

---

## 3. Test-Driven Development (TDD) Strategy

1. **Integration Tests (`crates/kestrel-core/tests/integration_tests.rs`)**:
   - `test_integration_synthetic_search_corpus_multi_page`:
     - Multi-occurrence keyword `KEYWORD_MULTI_OCCURRENCE` on Page 1 must return 2 distinct `SearchResult` entries.
     - Occurrence 0 must have `match_index_in_page: 0`, `global_match_index: 0`, and snippet containing "first mention".
     - Occurrence 1 must have `match_index_in_page: 1`, `global_match_index: 1`, and snippet containing "second mention".
2. **E2E Smoke Tests (`crates/kestrel-app/tests/smoke_test.rs`)**:
   - Test search navigation across individual occurrences on the same page.
   - Verify active match highlight vs regular match highlight color distinction.
   - Verify `X / N` counter formatting.
