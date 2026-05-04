// honkhonk-empty.jsx — First-run / empty state in C "Confetti" style

function EmptyState({ dark, frameW = 1180, frameH = 760 }) {
  const theme = dark ? C_DARK : C_LIGHT;

  // confetti dots — deterministic positions
  const dots = [
    { x: 8,  y: 12, tone: 'amber',  r: 18, rot: -8 },
    { x: 22, y: 78, tone: 'rose',   r: 22, rot: 12 },
    { x: 86, y: 18, tone: 'sky',    r: 24, rot: -6 },
    { x: 92, y: 70, tone: 'green',  r: 16, rot: 20 },
    { x: 14, y: 42, tone: 'violet', r: 14, rot: 4 },
    { x: 78, y: 48, tone: 'orange', r: 20, rot: -14 },
    { x: 50, y: 8,  tone: 'pink',   r: 14, rot: 8 },
    { x: 42, y: 88, tone: 'amber',  r: 16, rot: -10 },
  ];

  return (
    <div style={{
      width: frameW, height: frameH, background: theme.bg, color: theme.ink,
      fontFamily: '"Inter", ui-sans-serif, system-ui, sans-serif',
      borderRadius: 16, overflow: 'hidden', display: 'flex', flexDirection: 'column',
      border: `1px solid ${theme.hairline2}`, boxShadow: '0 30px 80px rgba(0,0,0,.22)',
      position: 'relative',
    }}>
      {/* floating confetti stickers */}
      {dots.map((d, i) => {
        const tone = C_TONE[d.tone];
        const tColor = `hsl(${tone.hue} ${tone.sat}% ${dark ? tone.light - 5 : tone.light}%)`;
        return (
          <div key={i} style={{
            position: 'absolute', left: `${d.x}%`, top: `${d.y}%`,
            width: d.r * 2, height: d.r * 2, borderRadius: d.r * 0.6,
            background: `radial-gradient(circle at 30% 25%, hsl(${tone.hue} ${tone.sat}% ${tone.light + 18}%), ${tColor})`,
            transform: `translate(-50%, -50%) rotate(${d.rot}deg)`,
            border: `1.5px solid hsl(${tone.hue} ${tone.sat}% ${dark ? 25 : 35}%)`,
            boxShadow: `inset 0 -3px 6px rgba(0,0,0,.18), inset 0 2px 0 rgba(255,255,255,.4), 0 6px 12px ${tColor}55`,
            opacity: 0.5,
          }}/>
        );
      })}

      {/* lightweight header so it still feels like the app */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '18px 24px', borderBottom: `1px solid ${theme.hairline}`, position: 'relative', zIndex: 2 }}>
        <div style={{
          width: 40, height: 40, borderRadius: 14,
          background: `conic-gradient(from 200deg at 60% 40%, ${theme.accent}, ${theme.accentDeep}, ${theme.accent})`,
          display: 'flex', alignItems: 'center', justifyContent: 'center',
          boxShadow: `0 4px 12px ${theme.accent}55, inset 0 1px 0 rgba(255,255,255,.5)`,
          transform: 'rotate(-5deg)',
        }}>
          <svg width="26" height="26" viewBox="0 0 64 64"><GooseGlyph color="#1a1208" accent="#fffbeb"/></svg>
        </div>
        <div style={{ fontSize: 22, fontWeight: 800, letterSpacing: '-0.025em', fontStyle: 'italic' }}>
          Honk<span style={{ color: theme.accent }}>Honk</span>
        </div>
        <div style={{ flex: 1 }}/>
        <span style={{ fontSize: 11, fontWeight: 800, color: theme.accentDeep, background: '#fef4a8', padding: '4px 10px', borderRadius: 999, letterSpacing: '.06em' }}>FIRST RUN</span>
      </div>

      {/* hero */}
      <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', padding: 40, position: 'relative', zIndex: 2 }}>
        <div style={{ maxWidth: 760, textAlign: 'center', display: 'flex', flexDirection: 'column', alignItems: 'center' }}>
          {/* big mascot */}
          <div style={{
            width: 160, height: 160, borderRadius: 56,
            background: `conic-gradient(from 200deg at 60% 40%, ${theme.accent}, ${theme.accentDeep}, ${theme.accent})`,
            display: 'flex', alignItems: 'center', justifyContent: 'center',
            boxShadow: `0 16px 40px ${theme.accent}66, inset 0 4px 0 rgba(255,255,255,.5), inset 0 -8px 0 rgba(0,0,0,.18)`,
            transform: 'rotate(-8deg)', marginBottom: 24, position: 'relative',
          }}>
            <svg width="120" height="120" viewBox="0 0 64 64"><GooseGlyph color="#1a1208" accent="#fffbeb"/></svg>
            {/* speech bubble */}
            <div style={{
              position: 'absolute', top: -14, right: -54, padding: '8px 14px',
              background: theme.paper, borderRadius: 18, border: `2px solid ${theme.ink}`,
              fontSize: 16, fontWeight: 800, fontStyle: 'italic', color: theme.ink,
              transform: 'rotate(8deg)', boxShadow: '0 4px 10px rgba(0,0,0,.15)',
            }}>HONK!</div>
          </div>

          <h1 style={{
            fontSize: 42, fontWeight: 800, margin: '0 0 12px', letterSpacing: '-.03em',
            fontStyle: 'italic', textWrap: 'balance', lineHeight: 1.05,
          }}>
            No sounds yet. <span style={{ color: theme.accent }}>Let's fix that.</span>
          </h1>
          <p style={{
            fontSize: 16, color: theme.inkDim, margin: '0 0 32px', lineHeight: 1.5, maxWidth: 480,
            textWrap: 'pretty', fontWeight: 500,
          }}>
            HonkHonk needs a folder of audio files to honk from. Drop one below, or pick from your machine.
          </p>

          {/* drop zone */}
          <div style={{
            width: 540, padding: '32px 28px', borderRadius: 22,
            background: theme.panel, border: `2.5px dashed ${theme.accent}`,
            display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 14,
            boxShadow: `0 0 0 6px ${theme.accent}15`, transform: 'rotate(-0.5deg)',
            position: 'relative',
          }}>
            <div style={{
              width: 56, height: 56, borderRadius: 18,
              background: `${theme.accent}22`, color: theme.accentDeep,
              display: 'flex', alignItems: 'center', justifyContent: 'center',
              border: `2px solid ${theme.accent}55`,
            }}>
              <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round">
                <path d="M3 7l2-2h6l2 2h8a1 1 0 0 1 1 1v11a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V8a1 1 0 0 1 1-1z"/>
                <path d="M12 11v6M9 14l3-3 3 3"/>
              </svg>
            </div>
            <div style={{ fontSize: 18, fontWeight: 800, color: theme.ink, letterSpacing: '-.015em' }}>
              Drop a folder here
            </div>
            <div style={{ fontSize: 12.5, color: theme.inkDim, lineHeight: 1.5, textAlign: 'center', maxWidth: 360 }}>
              MP3, WAV, OGG, FLAC — HonkHonk reads them all and watches the folder for new sounds.
            </div>
            <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginTop: 4 }}>
              <span style={{ fontSize: 11, color: theme.inkFaint, fontWeight: 700, letterSpacing: '.08em' }}>OR</span>
            </div>
            <button style={{
              height: 44, padding: '0 24px', borderRadius: 999,
              background: `linear-gradient(140deg, ${theme.accent}, ${theme.accentDeep})`,
              border: 'none', color: '#1a1208', fontSize: 14, fontWeight: 800, fontFamily: 'inherit', cursor: 'pointer',
              transform: 'rotate(-1deg)',
              boxShadow: `0 6px 16px ${theme.accent}66, inset 0 1px 0 rgba(255,255,255,.5), inset 0 -2px 0 rgba(0,0,0,.15)`,
            }}>Pick a folder…</button>
          </div>

          {/* portal permission notice */}
          <div style={{
            marginTop: 22, padding: '12px 16px', borderRadius: 12,
            background: dark ? 'rgba(56, 189, 248, 0.1)' : 'rgba(56, 189, 248, 0.12)',
            border: `1px solid ${dark ? 'rgba(56, 189, 248, 0.3)' : 'rgba(14, 116, 144, 0.3)'}`,
            display: 'flex', alignItems: 'center', gap: 12, fontSize: 12.5,
            color: dark ? '#7dd3fc' : '#0c4a6e', fontWeight: 600, maxWidth: 540,
          }}>
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" style={{ flexShrink: 0 }}>
              <rect x="4" y="11" width="16" height="10" rx="2"/>
              <path d="M8 11V8a4 4 0 0 1 8 0v3"/>
            </svg>
            <span style={{ textAlign: 'left', lineHeight: 1.5 }}>
              On first sound, KDE will ask if HonkHonk can register a virtual mic and global shortcuts. Say yes — we need both.
            </span>
          </div>

          {/* skip link */}
          <button style={{
            marginTop: 18, background: 'none', border: 'none', color: theme.inkDim,
            fontSize: 12.5, fontWeight: 600, cursor: 'pointer', textDecoration: 'underline', textUnderlineOffset: 3,
            fontFamily: 'inherit',
          }}>Or skip — I'll add sounds later</button>
        </div>
      </div>
    </div>
  );
}

window.EmptyState = EmptyState;
