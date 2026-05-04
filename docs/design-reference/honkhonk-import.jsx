// honkhonk-import.jsx — Bulk import review screen
// Shown after Iced just scanned a folder and found 47 sound files.
// Lets the user batch-tag, rename, exclude, and confirm before they hit the grid.

function BulkImport({ dark, frameW = 1180, frameH = 760 }) {
  const theme = dark ? C_DARK : C_LIGHT;

  // 12 fake import rows. Some pre-tagged by smart-detection, some need attention.
  const ROWS = [
    { fn: 'goose-honk-001.wav',          name: 'Goose honk',            cat: 'Honk',  tone: 'amber',  dur: '0:01', size: '24 KB',  status: 'ready', auto: true },
    { fn: 'goose-honk-002.wav',          name: 'Goose honk #2',         cat: 'Honk',  tone: 'amber',  dur: '0:01', size: '22 KB',  status: 'ready', auto: true },
    { fn: 'flock-of-geese.flac',         name: 'Flock of geese',        cat: 'Honk',  tone: 'amber',  dur: '0:04', size: '180 KB', status: 'ready', auto: true },
    { fn: 'vine_boom_HQ.mp3',            name: 'Vine boom',             cat: 'Meme',  tone: 'orange', dur: '0:02', size: '38 KB',  status: 'ready', auto: true },
    { fn: 'BRUH_(loud).ogg',             name: 'Bruh',                  cat: 'Meme',  tone: 'pink',   dur: '0:01', size: '14 KB',  status: 'review', auto: false, warn: 'Volume +6 dB hotter than peers' },
    { fn: 'oof.mp3',                     name: 'Oof',                   cat: 'Meme',  tone: 'red',    dur: '0:01', size: '10 KB',  status: 'ready', auto: true },
    { fn: 'fus_ro_dah.wav',              name: 'FUS RO DAH',            cat: 'Game',  tone: 'cyan',   dur: '0:02', size: '52 KB',  status: 'ready', auto: true },
    { fn: 'never gonna give .mp3',       name: 'Never gonna give',      cat: 'Music', tone: 'pink',   dur: '0:08', size: '125 KB', status: 'ready', auto: true },
    { fn: 'untitled (3).aiff',           name: '(needs a name)',        cat: '—',     tone: 'amber',  dur: '0:03', size: '430 KB', status: 'review', auto: false, warn: 'Generic filename — pick a name' },
    { fn: 'airhorn.mp3',                 name: 'Airhorn',               cat: 'Honk',  tone: 'yellow', dur: '0:02', size: '32 KB',  status: 'ready', auto: true },
    { fn: 'bonk_loop.opus',              name: 'Bonk',                  cat: 'Meme',  tone: 'lime',   dur: '0:00', size: '6 KB',   status: 'ready', auto: true },
    { fn: 'recording_2024-08-12.m4a',    name: 'Recording 2024-08-12',  cat: 'Custom', tone: 'gray',  dur: '0:14', size: '210 KB', status: 'skip', auto: false, warn: 'Silent first 1.4s — trim?' },
  ];

  const counts = {
    total: 47,
    shown: ROWS.length,
    ready: ROWS.filter(r => r.status === 'ready').length + 30, // pretend 30 more off-screen
    review: ROWS.filter(r => r.status === 'review').length + 4,
    skip: ROWS.filter(r => r.status === 'skip').length + 1,
  };

  return (
    <div style={{
      width: frameW, height: frameH, background: theme.bg, color: theme.ink,
      fontFamily: '"Inter", ui-sans-serif, system-ui, sans-serif',
      borderRadius: 16, overflow: 'hidden', display: 'flex', flexDirection: 'column',
      border: `1px solid ${theme.hairline2}`,
    }}>
      {/* Header */}
      <div style={{ display: 'flex', alignItems: 'flex-end', gap: 16, padding: '20px 28px 18px', borderBottom: `1px solid ${theme.hairline}` }}>
        <div style={{ flex: 1 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 4 }}>
            <span style={{ fontSize: 11, fontWeight: 800, color: theme.inkDim, letterSpacing: '.08em', textTransform: 'uppercase' }}>STEP 2 OF 2 · IMPORT REVIEW</span>
          </div>
          <div style={{ fontSize: 26, fontWeight: 800, letterSpacing: '-.025em', fontStyle: 'italic' }}>
            Found <span style={{ color: theme.accent }}>{counts.total} sounds</span> in <code style={{ fontSize: 16, fontWeight: 700, fontFamily: 'ui-monospace, monospace', background: 'transparent', padding: 0, color: theme.ink }}>~/Sounds/honkhonk</code>
          </div>
          <div style={{ fontSize: 12.5, color: theme.inkDim, marginTop: 6, fontWeight: 500 }}>
            Names and categories were auto-detected. Skim, fix anything that looks off, and add them to your library.
          </div>
        </div>

        {/* Status pills */}
        <div style={{ display: 'flex', gap: 8 }}>
          <StatusPill label="Ready"  count={counts.ready}  color="#16a34a" theme={theme}/>
          <StatusPill label="Review" count={counts.review} color="#f59e0b" theme={theme}/>
          <StatusPill label="Skip"   count={counts.skip}   color="#94a3b8" theme={theme}/>
        </div>
      </div>

      {/* Toolbar */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '12px 28px', borderBottom: `1px solid ${theme.hairline}`, background: theme.panel }}>
        <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 12, fontWeight: 600, color: theme.inkDim, cursor: 'pointer' }}>
          <span style={{ width: 14, height: 14, borderRadius: 3, background: theme.accent, border: `1px solid ${theme.accent}`, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
            <svg width="9" height="9" viewBox="0 0 24 24" fill="none" stroke="#1a1208" strokeWidth="3.5" strokeLinecap="round" strokeLinejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
          </span>
          <span style={{ color: theme.ink }}>{counts.shown} selected</span>
        </label>
        <div style={{ width: 1, height: 18, background: theme.hairline2, margin: '0 6px' }}/>

        <ToolbarBtn theme={theme} icon="tag">Set category…</ToolbarBtn>
        <ToolbarBtn theme={theme} icon="color">Color…</ToolbarBtn>
        <ToolbarBtn theme={theme} icon="vol">Normalize volume</ToolbarBtn>
        <ToolbarBtn theme={theme} icon="trim">Auto-trim silence</ToolbarBtn>

        <div style={{ flex: 1 }}/>

        <div style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '6px 10px', background: theme.bg, border: `1px solid ${theme.hairline2}`, borderRadius: 8, fontSize: 11.5, color: theme.inkDim }}>
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="11" cy="11" r="7"/><path d="M21 21l-4.3-4.3"/></svg>
          <input style={{ background: 'transparent', border: 'none', outline: 'none', fontSize: 12, color: theme.ink, width: 130, fontFamily: 'inherit' }} placeholder="Filter rows…" defaultValue=""/>
        </div>
      </div>

      {/* Table */}
      <div style={{ flex: 1, overflow: 'auto' }}>
        {/* table header */}
        <div style={{
          display: 'grid', gridTemplateColumns: '36px 36px 2.4fr 1.4fr 28px 0.7fr 0.6fr 0.7fr 80px',
          alignItems: 'center', gap: 12, padding: '8px 28px',
          fontSize: 10.5, fontWeight: 800, color: theme.inkFaint, letterSpacing: '.08em', textTransform: 'uppercase',
          borderBottom: `1px solid ${theme.hairline}`, background: theme.bg, position: 'sticky', top: 0, zIndex: 1,
        }}>
          <div></div>
          <div></div>
          <div>Name <span style={{ color: theme.inkFaint, fontWeight: 600, textTransform: 'none', letterSpacing: 'normal' }}>· source filename</span></div>
          <div>Category</div>
          <div></div>
          <div>Color</div>
          <div>Duration</div>
          <div>Size</div>
          <div style={{ textAlign: 'right' }}>Status</div>
        </div>

        {ROWS.map((r, i) => {
          const tone = C_TONE[r.tone] || C_TONE.amber;
          const tintBg = dark ? `hsl(${tone.hue} ${Math.min(40, tone.sat)}% 16%)` : `hsl(${tone.hue} ${Math.min(60, tone.sat)}% 95%)`;
          const tColor = `hsl(${tone.hue} ${tone.sat}% ${dark ? Math.max(50, tone.light) : tone.light - 5}%)`;
          const checked = r.status !== 'skip';
          const isWarn = !!r.warn;
          const fakeSound = { id: r.fn, name: r.name, tone: r.tone, seed: i, glyph: r.cat === 'Honk' ? 'goose-honk' : 'pop' };
          return (
            <div key={r.fn} style={{
              display: 'grid', gridTemplateColumns: '36px 36px 2.4fr 1.4fr 28px 0.7fr 0.6fr 0.7fr 80px',
              alignItems: 'center', gap: 12, padding: '10px 28px',
              borderBottom: `1px solid ${theme.hairline}`,
              background: i % 2 === 0 ? 'transparent' : (dark ? 'rgba(255,255,255,.015)' : 'rgba(0,0,0,.012)'),
              opacity: r.status === 'skip' ? 0.5 : 1,
            }}>
              {/* checkbox */}
              <span style={{
                width: 16, height: 16, borderRadius: 4,
                background: checked ? theme.accent : 'transparent',
                border: `1.5px solid ${checked ? theme.accent : theme.hairline2}`,
                display: 'flex', alignItems: 'center', justifyContent: 'center', cursor: 'pointer',
              }}>
                {checked && <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="#1a1208" strokeWidth="3.5" strokeLinecap="round" strokeLinejoin="round"><polyline points="20 6 9 17 4 12"/></svg>}
              </span>

              {/* sticker preview */}
              <div style={{ width: 28, height: 28, borderRadius: 8, background: tintBg, display: 'flex', alignItems: 'center', justifyContent: 'center', border: `1px solid ${theme.hairline}` }}>
                <span style={{ width: 14, height: 14, borderRadius: 14, background: tColor, boxShadow: 'inset 0 -2px 0 rgba(0,0,0,.15), inset 0 1px 0 rgba(255,255,255,.4)' }}/>
              </div>

              {/* name + filename */}
              <div style={{ minWidth: 0 }}>
                <div style={{ fontSize: 13, fontWeight: 700, color: theme.ink, letterSpacing: '-.005em', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', display: 'flex', alignItems: 'center', gap: 6 }}>
                  {r.name === '(needs a name)' ? <span style={{ color: '#dc2626', fontStyle: 'italic', fontWeight: 600 }}>{r.name}</span> : r.name}
                  {r.auto && <span style={{ fontSize: 9, fontWeight: 800, color: theme.accent, background: dark ? 'rgba(245,158,11,.15)' : 'rgba(245,158,11,.18)', padding: '1px 5px', borderRadius: 3, letterSpacing: '.05em' }}>AUTO</span>}
                </div>
                <div style={{ fontSize: 11, color: theme.inkDim, fontFamily: 'ui-monospace, monospace', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', marginTop: 1 }}>{r.fn}</div>
                {isWarn && (
                  <div style={{ fontSize: 11, color: '#b45309', marginTop: 3, display: 'flex', alignItems: 'center', gap: 4 }}>
                    <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><path d="M12 9v4M12 17v.01"/><circle cx="12" cy="12" r="9"/></svg>
                    {r.warn}
                  </div>
                )}
              </div>

              {/* category dropdown */}
              <div style={{
                fontSize: 12, fontWeight: 600, color: r.cat === '—' ? theme.inkFaint : theme.ink,
                padding: '5px 10px', background: theme.panel, border: `1px solid ${theme.hairline2}`,
                borderRadius: 6, display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 6, cursor: 'pointer',
              }}>
                <span>{r.cat}</span>
                <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" style={{ opacity: 0.5 }}><path d="M6 9l6 6 6-6"/></svg>
              </div>

              {/* preview button */}
              <button style={{ width: 26, height: 26, borderRadius: 6, background: theme.panel, border: `1px solid ${theme.hairline2}`, color: theme.ink, cursor: 'pointer', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                <svg width="11" height="11" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5.5v13l11-6.5z"/></svg>
              </button>

              {/* color swatch */}
              <span style={{ width: 18, height: 18, borderRadius: 6, background: tColor, border: `1.5px solid ${theme.hairline2}`, boxShadow: 'inset 0 1px 0 rgba(255,255,255,.3)' }}/>

              {/* duration */}
              <span style={{ fontSize: 12, color: theme.inkDim, fontFamily: 'ui-monospace, monospace' }}>{r.dur}</span>

              {/* size */}
              <span style={{ fontSize: 12, color: theme.inkDim, fontFamily: 'ui-monospace, monospace' }}>{r.size}</span>

              {/* status */}
              <div style={{ textAlign: 'right' }}>
                <RowStatus status={r.status} theme={theme}/>
              </div>
            </div>
          );
        })}

        {/* "+ 35 more" hint */}
        <div style={{
          padding: '14px 28px', textAlign: 'center', fontSize: 12, fontWeight: 600,
          color: theme.inkDim, background: theme.panel, borderTop: `1px solid ${theme.hairline}`,
        }}>
          ↓ <b style={{ color: theme.ink }}>{counts.total - counts.shown} more</b> below — scroll to review them all
        </div>
      </div>

      {/* Footer actions */}
      <div style={{
        display: 'flex', alignItems: 'center', gap: 12, padding: '14px 28px',
        borderTop: `1px solid ${theme.hairline2}`, background: theme.panel,
      }}>
        <div style={{ fontSize: 12, color: theme.inkDim, fontWeight: 500 }}>
          <b style={{ color: theme.ink }}>42 sounds</b> will be added to your library. <b style={{ color: '#dc2626' }}>5</b> skipped.
        </div>
        <div style={{ flex: 1 }}/>
        <button style={{
          height: 40, padding: '0 18px', borderRadius: 10,
          background: 'transparent', border: `1px solid ${theme.hairline2}`,
          fontSize: 13, fontWeight: 700, color: theme.inkDim, cursor: 'pointer', fontFamily: 'inherit',
        }}>Cancel</button>
        <button style={{
          height: 40, padding: '0 22px', borderRadius: 10,
          background: `linear-gradient(140deg, ${theme.accent}, ${theme.accentDeep})`,
          border: 'none', color: '#1a1208', fontSize: 13, fontWeight: 800,
          cursor: 'pointer', fontFamily: 'inherit', transform: 'rotate(-1deg)',
          boxShadow: `0 6px 16px ${theme.accent}55`, display: 'flex', alignItems: 'center', gap: 8,
        }}>
          <span>Add 42 sounds</span>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><path d="M5 12h14M13 6l6 6-6 6"/></svg>
        </button>
      </div>
    </div>
  );
}

function StatusPill({ label, count, color, theme }) {
  return (
    <div style={{
      display: 'flex', alignItems: 'center', gap: 6, padding: '6px 12px',
      background: theme.panel, border: `1px solid ${theme.hairline}`,
      borderRadius: 999, fontSize: 11.5, fontWeight: 700, color: theme.inkDim,
    }}>
      <span style={{ width: 8, height: 8, borderRadius: 8, background: color, boxShadow: `0 0 6px ${color}66` }}/>
      <span style={{ color: theme.ink }}>{count}</span>
      <span style={{ textTransform: 'uppercase', letterSpacing: '.05em', fontSize: 10.5 }}>{label}</span>
    </div>
  );
}

function ToolbarBtn({ theme, icon, children }) {
  const ICONS = {
    tag: <path d="M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82zM7 7h.01"/>,
    color: <><circle cx="12" cy="12" r="9"/><path d="M12 1v6M12 17v6M4.2 4.2l4.3 4.3"/></>,
    vol: <><path d="M11 5L6 9H2v6h4l5 4V5z"/><path d="M19.07 4.93a10 10 0 0 1 0 14.14M15.54 8.46a5 5 0 0 1 0 7.07"/></>,
    trim: <path d="M6 3v18M18 3v18M6 7h12M6 17h12"/>,
  };
  return (
    <button style={{
      height: 32, padding: '0 12px', borderRadius: 8,
      background: theme.bg, border: `1px solid ${theme.hairline2}`,
      fontSize: 11.5, fontWeight: 700, color: theme.ink, cursor: 'pointer',
      display: 'flex', alignItems: 'center', gap: 7, fontFamily: 'inherit',
    }}>
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" style={{ opacity: 0.7 }}>{ICONS[icon]}</svg>
      {children}
    </button>
  );
}

function RowStatus({ status, theme }) {
  if (status === 'ready') return (
    <span style={{ fontSize: 10.5, fontWeight: 800, color: '#15803d', background: 'rgba(22, 163, 74, 0.12)', padding: '3px 8px', borderRadius: 999, letterSpacing: '.05em' }}>READY</span>
  );
  if (status === 'review') return (
    <span style={{ fontSize: 10.5, fontWeight: 800, color: '#b45309', background: 'rgba(245, 158, 11, 0.15)', padding: '3px 8px', borderRadius: 999, letterSpacing: '.05em' }}>REVIEW</span>
  );
  return (
    <span style={{ fontSize: 10.5, fontWeight: 800, color: theme.inkDim, background: 'rgba(148, 163, 184, 0.18)', padding: '3px 8px', borderRadius: 999, letterSpacing: '.05em' }}>SKIP</span>
  );
}

window.BulkImport = BulkImport;
