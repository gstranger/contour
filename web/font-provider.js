// font-provider.js — font loading, measurement, and outline extraction using opentype.js
//
// Dependencies: opentype.js (loaded as <script src="https://unpkg.com/opentype.js@1.3.4/dist/opentype.min.js"></script>)

export class FontManager {
  constructor() {
    /** @type {Map<string, import('opentype.js').Font>} */
    this.fonts = new Map();

    /** @type {Map<string, Promise<import('opentype.js').Font>>} */
    this.loading = new Map();

    /** @type {string[]} */
    this.fallbackChain = ['sans-serif'];

    /**
     * Per-(font,size) glyph metrics cache.
     * Key: `family|weight|fontSize`
     * Value: Map<char, {width: number}>
     */
    this.glyphCache = new Map();
  }

  /**
   * Load a font from an ArrayBuffer.
   * @param {string} family
   * @param {ArrayBuffer} source
   * @param {number} [weight=400]
   * @param {'normal'|'italic'|'oblique'} [style='normal']
   */
  async load(family, source, weight = 400, style = 'normal') {
    const key = `${family}|${weight}|${style}`;
    if (this.fonts.has(key) || this.loading.has(key)) {
      return this.loading.get(key);
    }

    const promise = new Promise((resolve, reject) => {
      try {
        const font = opentype.parse(source);
        this.fonts.set(key, font);
        this.glyphCache.clear();
        resolve(font);
      } catch (e) {
        reject(e);
      }
    });

    this.loading.set(key, promise);
    return promise;
  }

  /**
   * Load a font from a URL.
   */
  async loadFromUrl(family, url, weight = 400, style = 'normal') {
    const resp = await fetch(url);
    const buf = await resp.arrayBuffer();
    return this.load(family, buf, weight, style);
  }

  unload(family) {
    for (const [key] of this.fonts) {
      if (key.startsWith(family + '|')) {
        this.fonts.delete(key);
      }
    }
    this.glyphCache.clear();
  }

  setFallbackChain(chain) {
    this.fallbackChain = chain;
    this.glyphCache.clear();
  }

  /**
   * Resolve a font family+weight+style to an opentype.Font.
   * Walks the fallback chain if exact match fails.
   * @returns {import('opentype.js').Font|null}
   */
  resolveFont(family, weight = 400, style = 'normal') {
    // 1. Exact match
    const exactKey = `${family}|${weight}|${style}`;
    if (this.fonts.has(exactKey)) {
      return this.fonts.get(exactKey);
    }

    // 2. Same family, closest weight
    let best = null;
    let bestDiff = Infinity;
    for (const [key, font] of this.fonts) {
      if (key.startsWith(family + '|')) {
        const parts = key.split('|');
        const w = parseInt(parts[1]) || 400;
        const diff = Math.abs(w - weight);
        if (diff < bestDiff) {
          bestDiff = diff;
          best = font;
        }
      }
    }
    if (best) return best;

    // 3. Walk fallback chain
    for (const fb of this.fallbackChain) {
      if (fb === family) continue;
      for (const [key, font] of this.fonts) {
        if (key.startsWith(fb + '|')) return font;
      }
    }

    return null;
  }

  /**
   * Get or compute glyph metrics for a character.
   */
  getGlyphMetrics(font, fontSize, char) {
    const cacheKey = `${font.names.fontFamily.en}|${font.tables.os2.usWeightClass}|${fontSize}`;
    if (!this.glyphCache.has(cacheKey)) {
      this.glyphCache.set(cacheKey, new Map());
    }
    const cache = this.glyphCache.get(cacheKey);
    if (cache.has(char)) {
      return cache.get(char);
    }

    const scale = fontSize / font.unitsPerEm;
    const glyphIndex = font.charToGlyphIndex(char);
    const glyph = font.glyphs.get(glyphIndex);
    const width = (glyph ? glyph.advanceWidth : font.unitsPerEm * 0.5) * scale;

    const metrics = { width };
    cache.set(char, metrics);
    return metrics;
  }
}

export class FontProvider {
  /**
   * @param {FontManager} fontManager
   */
  constructor(fontManager) {
    this.fontManager = fontManager;
  }

  /**
   * Measure text content with the given style.
   * @param {string} content
   * @param {{font_family: string, font_size: number, font_weight: number, font_style: number, letter_spacing: number, line_height: number}} style
   * @returns {{char_widths: number[], line_height: number, ascent: number, descent: number, total_width: number}}
   */
  measure(content, style) {
    const font = this.fontManager.resolveFont(
      style.font_family || 'sans-serif',
      style.font_weight || 400,
      ['normal', 'italic', 'oblique'][style.font_style || 0] || 'normal'
    );

    const fontSize = style.font_size || 16;
    const letterSpacingPx = (style.letter_spacing || 0) * fontSize;

    // If no font available, return approximate metrics
    if (!font) {
      const charWidths = [...content].map(() => fontSize * 0.6);
      const totalWidth = charWidths.reduce((s, w) => s + w + letterSpacingPx, 0) - letterSpacingPx;
      return {
        char_widths: charWidths,
        line_height: fontSize * (style.line_height || 1.2),
        ascent: fontSize * 0.72,
        descent: fontSize * 0.28,
        total_width: Math.max(0, totalWidth),
      };
    }

    const scale = fontSize / font.unitsPerEm;
    const charWidths = [];
    let totalWidth = 0;

    for (const ch of [...content]) {
      if (ch === '\n') {
        charWidths.push(0);
        continue;
      }
      const metrics = this.fontManager.getGlyphMetrics(font, fontSize, ch);
      charWidths.push(metrics.width + letterSpacingPx);
      totalWidth += metrics.width + letterSpacingPx;
    }

    const lineHeight = fontSize * (style.line_height || 1.2);
    const ascent = font.ascender * scale;
    const descent = -font.descender * scale;

    return {
      char_widths: charWidths,
      line_height: lineHeight,
      ascent: ascent,
      descent: descent,
      total_width: totalWidth,
    };
  }

  /**
   * Generate glyph outlines for text-to-outlines conversion.
   * @param {string} content
   * @param {object} style
   * @returns {import('./types').GlyphOutline[]}
   */
  getOutlines(content, style) {
    const font = this.fontManager.resolveFont(
      style.font_family || 'sans-serif',
      style.font_weight || 400,
      ['normal', 'italic', 'oblique'][style.font_style || 0] || 'normal'
    );
    if (!font) return [];

    const outlines = [];
    const size = style.font_size || 16;

    for (const ch of [...content]) {
      const glyphIndex = font.charToGlyphIndex(ch);
      const glyph = font.glyphs.get(glyphIndex);
      if (!glyph) {
        outlines.push({
          char: ch,
          advance_width: size * 0.5,
          paths: [],
        });
        continue;
      }

      const path = glyph.getPath(0, 0, size);
      const commands = [];

      for (const cmd of path.commands) {
        switch (cmd.type) {
          case 'M':
            commands.push({ MoveTo: [cmd.x, cmd.y] });
            break;
          case 'L':
            commands.push({ LineTo: [cmd.x, cmd.y] });
            break;
          case 'Q':
            commands.push({ QuadTo: [cmd.x1, cmd.y1, cmd.x, cmd.y] });
            break;
          case 'C':
            commands.push({ CubicTo: [cmd.x1, cmd.y1, cmd.x2, cmd.y2, cmd.x, cmd.y] });
            break;
          case 'Z':
            commands.push('Close');
            break;
        }
      }

      outlines.push({
        char: ch,
        advance_width: glyph.advanceWidth * (size / font.unitsPerEm),
        paths: [{ commands }],
      });
    }

    return outlines;
  }
}
