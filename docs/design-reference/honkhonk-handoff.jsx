// honkhonk-handoff.jsx — Rust handoff artboard
// Renders a syntax-highlighted, scrollable preview of theme.rs / sound_tile.rs / mod.rs
// so the user can read them inside the design canvas without leaving the file.

function HandoffPanel({ dark, frameW = 1180, frameH = 760 }) {
  const theme = dark ? C_DARK : C_LIGHT;
  const [active, setActive] = React.useState('theme.rs');
  const [files, setFiles] = React.useState({ 'theme.rs': null, 'sound_tile.rs': null, 'mod.rs': null });

  React.useEffect(() => {
    const fetchOne = async (key, path) => {
      try {
        const r = await fetch(path);
        const txt = await r.text();
        setFiles(f => ({ ...f, [key]: txt }));
      } catch (e) {
        setFiles(f => ({ ...f, [key]: '// could not load — try opening the file directly' }));
      }
    };
    fetchOne('theme.rs',      'src-rust/ui/theme.rs');
    fetchOne('sound_tile.rs', 'src-rust/ui/sound_tile.rs');
    fetchOne('mod.rs',        'src-rust/ui/mod.rs');
  }, []);

  const FILES = [
    { id: 'mod.rs',        path: 'src/ui/mod.rs',         lines: () => (files['mod.rs']        || '').split('\n').length },
    { id: 'theme.rs',      path: 'src/ui/theme.rs',       lines: () => (files['theme.rs']      || '').split('\n').length },
    { id: 'sound_tile.rs', path: 'src/ui/sound_tile.rs',  lines: () => (files['sound_tile.rs'] || '').split('\n').length },
  ];

  const codeBg   = dark ? '#0f0d0a' : '#1f1c16';
  const codeInk  = '#fbf3df';
  const codeDim  = '#a39377';
  const codeKey  = '#fbbf24';
  const codeStr  = '#86efac';
  const codeCom  = '#6a5b46';
  const codeFn   = '#7dd3fc';
  const codeNum  = '#fb923c';

  // Tiny tokenizer for Rust → spans. Not bulletproof; good enough for preview.
  function highlight(src) {
    if (!src) return null;
    const KW = /\b(fn|let|mut|pub|use|crate|struct|enum|impl|trait|for|in|if|else|match|return|self|Self|as|mod|const|static|where|type|ref|move|true|false|None|Some|Ok|Err)\b/g;
    const lines = src.split('\n');
    return lines.map((ln, i) => {
      // Tokenize this line into segments
      const out = [];
      let rest = ln;
      let key = 0;
      const push = (text, color) => {
        if (!text) return;
        out.push(<span key={key++} style={color ? { color } : undefined}>{text}</span>);
      };
      // Comment first
      const commentIdx = rest.indexOf('//');
      let pre = rest, comment = '';
      if (commentIdx >= 0) {
        pre = rest.slice(0, commentIdx);
        comment = rest.slice(commentIdx);
      }
      // Strings (basic " support)
      const segments = pre.split(/("[^"]*")/g);
      for (const seg of segments) {
        if (!seg) continue;
        if (seg.startsWith('"') && seg.endsWith('"')) {
          push(seg, codeStr);
        } else {
          // Keywords + numbers + identifiers
          let lastIdx = 0;
          const re = new RegExp(`(${KW.source.slice(2, -2)})|\\b(\\d+(?:\\.\\d+)?)\\b|\\b([a-z_][a-zA-Z0-9_]*)\\s*(?=\\()|#\\[[^\\]]*\\]`, 'g');
          let m;
          while ((m = re.exec(seg)) !== null) {
            push(seg.slice(lastIdx, m.index));
            if (m[1]) push(m[1], codeKey);
            else if (m[2]) push(m[2], codeNum);
            else if (m[3]) push(m[3], codeFn);
            else push(m[0], codeCom);
            lastIdx = m.index + m[0].length;
          }
          push(seg.slice(lastIdx));
        }
      }
      if (comment) push(comment, codeCom);

      return (
        <div key={i} style={{ display: 'flex', minHeight: 19 }}>
          <span style={{ width: 38, color: '#4a4030', textAlign: 'right', paddingRight: 12, userSelect: 'none', flexShrink: 0, fontVariantNumeric: 'tabular-nums' }}>{i + 1}</span>
          <span style={{ flex: 1, whiteSpace: 'pre' }}>{out.length ? out : ' '}</span>
        </div>
      );
    });
  }

  const activeContent = files[active];
  const activeFile = FILES.find(f => f.id === active);

  return (
    <div style={{
      width: frameW, height: frameH, background: theme.bg, color: theme.ink,
      fontFamily: '"Inter", ui-sans-serif, system-ui, sans-serif',
      borderRadius: 16, overflow: 'hidden', display: 'flex', flexDirection: 'column',
      border: `1px solid ${theme.hairline2}`,
    }}>
      {/* Header */}
      <div style={{ padding: '20px 28px 16px', borderBottom: `1px solid ${theme.hairline}` }}>
        <div style={{ display: 'flex', alignItems: 'baseline', gap: 12 }}>
          <span style={{ fontSize: 11, fontWeight: 800, color: theme.inkDim, letterSpacing: '.08em', textTransform: 'uppercase' }}>HANDOFF · src/ui/</span>
          <span style={{ fontSize: 11, color: theme.inkFaint, fontWeight: 600 }}>· drop these into your Iced project root</span>
        </div>
        <div style={{ fontSize: 22, fontWeight: 800, letterSpacing: '-.02em', fontStyle: 'italic', marginTop: 4 }}>
          Rust starters — <span style={{ color: theme.accent }}>theme.rs</span> + <span style={{ color: theme.accent }}>sound_tile.rs</span>
        </div>
        <div style={{ fontSize: 12.5, color: theme.inkDim, marginTop: 6, lineHeight: 1.5, maxWidth: 800 }}>
          Confetti palette as a real Iced <code>Theme</code> enum with <code>Hh</code> trait + per-sound <code>Tone</code>, and the sticker tile as a <code>canvas::Program</code> with the same -3°…+3° rotation logic, radial gloss, and glyph primitives the mockup uses. Includes goose, angry-goose, boom, note, arrow, scream, star, and dot glyphs.
        </div>
      </div>

      <div style={{ flex: 1, display: 'flex', overflow: 'hidden' }}>
        {/* File tree */}
        <div style={{ width: 220, borderRight: `1px solid ${theme.hairline}`, background: theme.panel, padding: '14px 12px', display: 'flex', flexDirection: 'column', gap: 4 }}>
          <div style={{ fontSize: 10.5, fontWeight: 800, color: theme.inkFaint, letterSpacing: '.08em', padding: '4px 10px 8px' }}>FILES</div>
          {FILES.map(f => {
            const sel = f.id === active;
            return (
              <button key={f.id} onClick={() => setActive(f.id)} style={{
                display: 'flex', alignItems: 'center', gap: 8, padding: '8px 10px',
                background: sel ? (dark ? 'rgba(245,158,11,.12)' : 'rgba(245,158,11,.15)') : 'transparent',
                border: 'none', borderRadius: 8, cursor: 'pointer', textAlign: 'left',
                color: sel ? theme.accent : theme.ink, fontFamily: 'inherit',
                fontSize: 12.5, fontWeight: sel ? 800 : 600,
              }}>
                <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" style={{ opacity: sel ? 1 : 0.55, flexShrink: 0 }}>
                  <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
                  <path d="M14 2v6h6"/>
                </svg>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontFamily: 'ui-monospace, monospace', fontSize: 12, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{f.id}</div>
                  <div style={{ fontFamily: 'ui-monospace, monospace', fontSize: 10, color: theme.inkFaint, marginTop: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{f.path}</div>
                </div>
                {files[f.id] && <span style={{ fontSize: 9.5, color: theme.inkFaint, fontFamily: 'ui-monospace, monospace' }}>{f.lines()}L</span>}
              </button>
            );
          })}

          <div style={{ marginTop: 'auto', padding: '12px 10px 4px', borderTop: `1px solid ${theme.hairline}`, fontSize: 11, color: theme.inkDim, lineHeight: 1.5 }}>
            <b style={{ color: theme.ink }}>Cargo.toml deps</b>
            <pre style={{ margin: '6px 0 0', fontFamily: 'ui-monospace, monospace', fontSize: 10.5, color: theme.inkDim, whiteSpace: 'pre-wrap' }}>
{`iced = { version = "0.13",
    features = ["canvas",
                "advanced",
                "tokio"] }`}
            </pre>
          </div>
        </div>

        {/* Code panel */}
        <div style={{ flex: 1, background: codeBg, color: codeInk, position: 'relative', overflow: 'hidden', display: 'flex', flexDirection: 'column' }}>
          {/* file tab strip */}
          <div style={{
            display: 'flex', alignItems: 'center', gap: 0, height: 34,
            background: '#0a0907', borderBottom: '1px solid rgba(255,255,255,.06)',
            paddingLeft: 8,
          }}>
            <div style={{
              padding: '0 14px', height: '100%', display: 'flex', alignItems: 'center', gap: 8,
              background: codeBg, fontSize: 11.5, fontFamily: 'ui-monospace, monospace',
              color: codeInk, fontWeight: 600, borderRight: '1px solid rgba(255,255,255,.06)',
            }}>
              <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke={codeKey} strokeWidth="2.5">
                <circle cx="12" cy="12" r="9"/>
                <path d="M12 7v5l3 2"/>
              </svg>
              {activeFile.path}
            </div>
            <div style={{ flex: 1 }}/>
            <span style={{ fontSize: 10.5, color: codeDim, fontFamily: 'ui-monospace, monospace', padding: '0 14px' }}>rust · UTF-8 · LF</span>
          </div>

          {/* code body */}
          <div style={{ flex: 1, overflow: 'auto', padding: '14px 0 14px 14px', fontFamily: 'ui-monospace, "JetBrains Mono", monospace', fontSize: 12, lineHeight: '19px' }}>
            {activeContent === null && <div style={{ color: codeDim, padding: 12 }}>Loading…</div>}
            {activeContent !== null && highlight(activeContent)}
          </div>
        </div>
      </div>
    </div>
  );
}

window.HandoffPanel = HandoffPanel;
