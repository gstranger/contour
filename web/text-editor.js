// text-editor.js — keyboard input, cursor, selection, and IME handling for inline text editing
//
// The TextEditor manages editing state in JS (cursor position, selection range,
// IME composition). It calls back into the engine (via the Graph WASM instance) to
// persist committed text changes. The engine never sees partial IME strings.

export class TextEditor {
  /**
   * @param {*} graph — the WASM Graph instance
   * @param {import('./font-provider').FontProvider} fontProvider
   */
  constructor(graph, fontProvider) {
    this.graph = graph;
    this.fontProvider = fontProvider;

    /** @type {number|null} */
    this.textId = null;

    /** @type {'idle'|'selected'|'editing'} */
    this.state = 'idle';

    /** @type {number} — char index within content */
    this.cursorPosition = 0;

    /** @type {number|null} — if set, selection extends from anchor to cursor */
    this.selectionAnchor = null;

    /** @type {boolean} */
    this.composing = false;
    /** @type {string} */
    this.compositionText = '';
  }

  edit(id) {
    this.textId = id;
    this.state = 'editing';
    this.cursorPosition = 0;
    this.selectionAnchor = null;
    this.composing = false;
    this.compositionText = '';
  }

  select(id) {
    this.textId = id;
    this.state = 'selected';
    this.cursorPosition = 0;
    this.selectionAnchor = null;
  }

  deselect() {
    this.textId = null;
    this.state = 'idle';
    this.cursorPosition = 0;
    this.selectionAnchor = null;
    this.composing = false;
  }

  /**
   * Commit a content change to the engine and trigger re-measure.
   */
  commit(newContent) {
    if (this.textId === null) return;

    this.graph.set_text_content(this.textId, newContent);

    // Trigger immediate re-measure so next render frame has fresh metrics
    const text = this.graph.get_text(this.textId);
    if (text && text.style) {
      const metrics = this.fontProvider.measure(newContent, text.style);
      this.graph.set_text_metrics(this.textId, metrics);
    }
  }

  getContent() {
    if (this.textId === null) return '';
    const text = this.graph.get_text(this.textId);
    return text ? text.content : '';
  }

  /**
   * Handle keyboard input. Call from the canvas keydown handler when state === 'editing'.
   */
  onKeyDown(event) {
    if (this.state !== 'editing' || this.textId === null) return;

    const content = this.getContent();
    const chars = [...content];
    let pos = this.cursorPosition;
    let anchor = this.selectionAnchor;

    const hasSelection = anchor !== null && anchor !== pos;
    const selStart = hasSelection ? Math.min(pos, anchor) : pos;
    const selEnd = hasSelection ? Math.max(pos, anchor) : pos;

    const insert = (text) => {
      const before = chars.slice(0, selStart).join('');
      const after = chars.slice(selEnd).join('');
      this.commit(before + text + after);
      const newPos = selStart + [...text].length;
      this.cursorPosition = newPos;
      this.selectionAnchor = null;
    };

    const deleteAt = (offset, dir) => {
      if (hasSelection) { insert(''); return; }
      const idx = pos + offset;
      if (idx < 0 || idx > chars.length) return;
      const before = chars.slice(0, idx).join('');
      const after = chars.slice(idx + 1).join('');
      this.commit(before + after);
      this.cursorPosition = Math.max(0, pos + dir);
      this.selectionAnchor = null;
    };

    const metaOrCtrl = event.metaKey || event.ctrlKey;

    switch (event.key) {
      case 'ArrowLeft':
        event.preventDefault();
        if (metaOrCtrl) {
          // Word left: find previous word boundary
          let p = Math.max(0, selStart - 1);
          while (p > 0 && chars[p] === ' ') p--;
          while (p > 0 && chars[p - 1] !== ' ') p--;
          pos = p;
        } else {
          pos = Math.max(0, selStart - 1);
        }
        if (event.shiftKey) {
          anchor = anchor ?? this.cursorPosition;
        } else {
          anchor = null;
        }
        this.cursorPosition = pos;
        this.selectionAnchor = anchor;
        break;

      case 'ArrowRight':
        event.preventDefault();
        if (metaOrCtrl) {
          let p = selEnd;
          while (p < chars.length && chars[p] === ' ') p++;
          while (p < chars.length && chars[p] !== ' ') p++;
          pos = p;
        } else {
          pos = Math.min(chars.length, selEnd + 1);
        }
        if (event.shiftKey) {
          anchor = anchor ?? this.cursorPosition;
        } else {
          anchor = null;
        }
        this.cursorPosition = pos;
        this.selectionAnchor = anchor;
        break;

      case 'Backspace':
        event.preventDefault();
        deleteAt(-1, -1);
        break;

      case 'Delete':
        event.preventDefault();
        deleteAt(0, 0);
        break;

      case 'Enter':
        event.preventDefault();
        insert('\n');
        break;

      case 'Escape':
        event.preventDefault();
        this.deselect();
        break;

      case 'a':
        if (metaOrCtrl) {
          event.preventDefault();
          this.cursorPosition = 0;
          this.selectionAnchor = chars.length;
        }
        break;

      case 'c':
      case 'x':
        if (metaOrCtrl && hasSelection) {
          event.preventDefault();
          const selected = chars.slice(selStart, selEnd).join('');
          navigator.clipboard.writeText(selected).catch(() => {});
          if (event.key === 'x') { insert(''); }
        }
        break;

      case 'v':
        if (metaOrCtrl) {
          event.preventDefault();
          navigator.clipboard.readText().then((text) => {
            if (this.state === 'editing' && this.textId !== null) {
              insert(text);
            }
          }).catch(() => {});
        }
        break;

      default:
        if (!this.composing && event.key.length === 1 && !metaOrCtrl) {
          event.preventDefault();
          insert(event.key);
        }
        break;
    }
  }

  /**
   * Handle mouse click on text — position the cursor.
   */
  onMouseDown(x, y) {
    if (this.textId === null || this.state !== 'editing') return;

    const hit = this.graph.get_text_hit(this.textId, x, y);
    if (hit) {
      this.cursorPosition = hit[0];
      this.selectionAnchor = null;
    }
  }

  /**
   * Handle mouse drag to extend selection.
   */
  onMouseDrag(x, y) {
    if (this.textId === null || this.state !== 'editing') return;

    const hit = this.graph.get_text_hit(this.textId, x, y);
    if (hit) {
      if (this.selectionAnchor === null) {
        this.selectionAnchor = this.cursorPosition;
      }
      this.cursorPosition = hit[0];
    }
  }

  // --- IME handlers ---

  onCompositionStart() {
    this.composing = true;
    this.compositionText = '';
  }

  onCompositionUpdate(text) {
    this.compositionText = text;
  }

  onCompositionEnd(text) {
    this.compositionText = '';
    this.composing = false;
    if (text) {
      const content = this.getContent();
      const chars = [...content];
      const hasSelection = this.selectionAnchor !== null && this.selectionAnchor !== this.cursorPosition;
      const selStart = hasSelection ? Math.min(this.cursorPosition, this.selectionAnchor) : this.cursorPosition;
      const selEnd = hasSelection ? Math.max(this.cursorPosition, this.selectionAnchor) : this.cursorPosition;

      const before = chars.slice(0, selStart).join('');
      const after = chars.slice(selEnd).join('');
      this.commit(before + text + after);
      this.cursorPosition = selStart + [...text].length;
      this.selectionAnchor = null;
    }
  }
}