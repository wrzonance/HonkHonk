// honkhonk-direction-c.jsx — "Confetti" v2
// Pushed harder: hand-drawn wonkiness, mascot moments, sticker thumbnails
// replacing waveforms, Favorites tab, more rhythm and personality.

const C_LIGHT = {
  bg:        '#f4efe4',
  panel:     '#fffaf0',
  panelDeep: '#ebe4d3',
  ink:       '#1a1409',
  inkDim:    '#766a52',
  inkFaint:  '#a89c80',
  hairline:  'rgba(26, 20, 9, 0.08)',
  hairline2: 'rgba(26, 20, 9, 0.16)',
  accent:    '#f59e0b',
  accentDeep:'#b45309',
  good:      '#16a34a',
  paper:     '#fffdf6',
};

const C_DARK = {
  bg:        '#171410',
  panel:     '#1f1c16',
  panelDeep: '#100e0a',
  ink:       '#fbf3df',
  inkDim:    '#aea08a',
  inkFaint:  '#6e6453',
  hairline:  'rgba(251, 243, 223, 0.08)',
  hairline2: 'rgba(251, 243, 223, 0.16)',
  accent:    '#fbbf24',
  accentDeep:'#f59e0b',
  good:      '#4ade80',
  paper:     '#231e16',
};

const C_TONE = {
  amber:  { hue: 38,  sat: 95, light: 55 },
  orange: { hue: 22,  sat: 90, light: 56 },
  rose:   { hue: 350, sat: 80, light: 58 },
  pink:   { hue: 322, sat: 75, light: 60 },
  red:    { hue: 5,   sat: 80, light: 56 },
  violet: { hue: 268, sat: 70, light: 62 },
  sky:    { hue: 205, sat: 85, light: 55 },
  green:  { hue: 145, sat: 65, light: 48 },
  slate:  { hue: 220, sat: 12, light: 50 },
};
function ctone(t, l, dark) {
  const o = C_TONE[t] || C_TONE.amber;
  return `hsl(${o.hue} ${o.sat}% ${dark ? l - 8 : l}%)`;
}

// ─── Hand-drawn thumbnail glyphs — one per sound, picked by tone+seed ─────
// SVG icons painted on the sticker. Slightly off-axis to feel hand-stickered.
const C_GLYPHS = {
  'vine-boom':    (c) => <g><circle cx="32" cy="32" r="14" fill={c.fg}/><circle cx="32" cy="32" r="22" fill="none" stroke={c.fg} strokeWidth="2.5" strokeDasharray="2 4" opacity=".7"/></g>,
  'bruh':         (c) => <text x="32" y="40" textAnchor="middle" fontSize="22" fontWeight="800" fill={c.fg} fontFamily="Inter">¯\\_(ツ)</text>,
  'oof':          (c) => <g><path d="M16 38c4 4 8 6 16 6s12-2 16-6" stroke={c.fg} strokeWidth="3" fill="none" strokeLinecap="round"/><circle cx="22" cy="24" r="3" fill={c.fg}/><circle cx="42" cy="24" r="3" fill={c.fg}/></g>,
  'sad-violin':   (c) => <g><path d="M22 18l4 28c.5 4 4 6 6 6s5.5-2 6-6l4-28" stroke={c.fg} strokeWidth="2.5" fill="none" strokeLinecap="round"/><path d="M28 28h8M28 36h8" stroke={c.fg} strokeWidth="1.5"/></g>,
  'metal-pipe':   (c) => <g><rect x="20" y="14" width="10" height="34" rx="2" fill={c.fg}/><path d="M14 50l28-4" stroke={c.fg} strokeWidth="2" strokeDasharray="3 3"/></g>,
  'airhorn':      (c) => <g><path d="M14 24h12l16-8v32l-16-8H14z" fill={c.fg}/><path d="M44 20l6-2M46 32h6M44 44l6 2" stroke={c.fg} strokeWidth="2.5" strokeLinecap="round"/></g>,

  'discord-call': (c) => <path d="M18 24c0-3 2-5 5-5h18c3 0 5 2 5 5v12c0 3-2 5-5 5h-8l-7 6v-6h-3c-3 0-5-2-5-5z" fill={c.fg}/>,
  'discord-leave':(c) => <g><path d="M18 24c0-3 2-5 5-5h12c3 0 5 2 5 5v12c0 3-2 5-5 5H23c-3 0-5-2-5-5z" fill={c.fg}/><path d="M44 20l8 12-8 12" stroke={c.fg} strokeWidth="3" strokeLinecap="round" strokeLinejoin="round" fill="none"/></g>,
  'wow':          (c) => <text x="32" y="42" textAnchor="middle" fontSize="22" fontWeight="800" fill={c.fg} fontFamily="Inter">WOW!</text>,
  'aughhh':       (c) => <g><path d="M18 26q14-10 28 0" stroke={c.fg} strokeWidth="2.5" fill="none"/><circle cx="22" cy="34" r="3" fill={c.fg}/><circle cx="42" cy="34" r="3" fill={c.fg}/><path d="M22 44q10 6 20 0" stroke={c.fg} strokeWidth="2.5" fill="none" strokeLinecap="round"/></g>,

  'hello-there':  (c) => <text x="32" y="40" textAnchor="middle" fontSize="13" fontWeight="800" fill={c.fg} fontFamily="Inter">HELLO</text>,
  'fus-ro-dah':   (c) => <g><path d="M14 32h36" stroke={c.fg} strokeWidth="3" strokeLinecap="round"/><path d="M22 24l-8 8 8 8M42 24l8 8-8 8" stroke={c.fg} strokeWidth="3" strokeLinecap="round" strokeLinejoin="round" fill="none"/></g>,
  'wasted':       (c) => <text x="32" y="40" textAnchor="middle" fontSize="11" fontWeight="800" fill={c.fg} fontFamily="Inter" letterSpacing="2">WASTED</text>,
  'mission-pass': (c) => <g><circle cx="32" cy="32" r="16" fill="none" stroke={c.fg} strokeWidth="3"/><path d="M24 32l6 6 12-12" stroke={c.fg} strokeWidth="3" strokeLinecap="round" strokeLinejoin="round" fill="none"/></g>,

  'rickroll':     (c) => <g><path d="M22 18v28a4 4 0 0 0 4 4h0a4 4 0 0 0 4-4V22h12v22a4 4 0 0 0 4 4h0a4 4 0 0 0 4-4V14L22 18z" fill={c.fg}/></g>,
  'mii-channel':  (c) => <g><circle cx="22" cy="32" r="8" fill={c.fg}/><circle cx="42" cy="32" r="8" fill={c.fg}/><path d="M22 38v6M42 38v6" stroke={c.fg} strokeWidth="2.5"/></g>,
  'all-star':     (c) => <path d="M32 14l5 12 13 1-10 9 3 13-11-7-11 7 3-13-10-9 13-1z" fill={c.fg}/>,
  'never-gonna':  (c) => <g><path d="M14 32h28M34 22l8 10-8 10" stroke={c.fg} strokeWidth="3" strokeLinecap="round" strokeLinejoin="round" fill="none"/><circle cx="50" cy="20" r="3" fill={c.fg}/></g>,

  'goose-honk':   (c) => <GooseGlyph color={c.fg} accent={c.bg}/>,
  'goose-honk-2': (c) => <GooseGlyph color={c.fg} accent={c.bg} angry/>,
  'goose-flock':  (c) => <g><GooseGlyph color={c.fg} accent={c.bg} small offsetX={-8} offsetY={-4}/><GooseGlyph color={c.fg} accent={c.bg} small offsetX={8} offsetY={6}/></g>,

  'bonk':         (c) => <g><path d="M18 28l8-8 6 6 12-12 4 4-12 12 6 6-8 8-16-16z" fill={c.fg}/></g>,
  'pop':          (c) => <g><circle cx="32" cy="32" r="10" fill={c.fg}/><path d="M14 14l6 6M50 14l-6 6M14 50l6-6M50 50l-6-6" stroke={c.fg} strokeWidth="3" strokeLinecap="round"/></g>,
  'whoosh':       (c) => <g><path d="M14 24q10-4 22 0t14 0" stroke={c.fg} strokeWidth="3" fill="none" strokeLinecap="round"/><path d="M14 34q10-4 22 0t14 0" stroke={c.fg} strokeWidth="3" fill="none" strokeLinecap="round" opacity=".7"/><path d="M14 44q10-4 22 0t14 0" stroke={c.fg} strokeWidth="3" fill="none" strokeLinecap="round" opacity=".4"/></g>,
};

function GooseGlyph({ color, accent, angry, small, offsetX = 0, offsetY = 0 }) {
  const s = small ? 0.7 : 1;
  return (
    <g transform={`translate(${offsetX} ${offsetY}) scale(${s}) translate(${(1-s)*32/s} ${(1-s)*32/s})`}>
      <ellipse cx="28" cy="42" rx="14" ry="10" fill={color}/>
      <path d="M38 18c4 0 7 3 7 7v6c0 2-1 3.5-2.5 4.5l-6 4c-1 .8-2.5 1-3.5.5l-3-1V26c0-1 .4-2 1-2.7l2.8-3v-1.3c0-.5.4-1 1-1z" fill={color}/>
      <path d="M46 22l5 1.5c.6.2.8 1 .3 1.4l-4.5 2.4c-.5.3-1 0-1-.5v-4c0-.4.2-.5.3-.5z" fill={accent}/>
      <circle cx="40" cy="22" r="1.4" fill="#1a1208"/>
      {angry && <path d="M36 16l4 3M44 14l-2 4" stroke="#1a1208" strokeWidth="1.5" strokeLinecap="round"/>}
    </g>
  );
}

// ─── Sticker thumbnail ────────────────────────────────────────────────────
function CSticker({ s, dark, size = 56, rotation = 0 }) {
  const tone = C_TONE[s.tone] || C_TONE.amber;
  const stickerBg = `hsl(${tone.hue} ${tone.sat}% ${dark ? Math.max(40, tone.light - 5) : tone.light}%)`;
  const stickerHi = `hsl(${tone.hue} ${tone.sat}% ${dark ? Math.min(70, tone.light + 12) : Math.min(85, tone.light + 22)}%)`;
  const fg = '#1a1208';
  const Glyph = C_GLYPHS[s.id];
  return (
    <div style={{
      width: size, height: size, borderRadius: size * 0.28,
      background: `radial-gradient(circle at 30% 25%, ${stickerHi}, ${stickerBg} 65%)`,
      boxShadow: `inset 0 -3px 6px rgba(0,0,0,.18), inset 0 2px 0 rgba(255,255,255,.4), 0 4px 10px ${stickerBg}66`,
      transform: `rotate(${rotation}deg)`, position: 'relative', flexShrink: 0,
      border: `1.5px solid hsl(${tone.hue} ${tone.sat}% ${dark ? 25 : 35}%)`,
      overflow: 'hidden',
    }}>
      <svg width={size} height={size} viewBox="0 0 64 64" style={{ display: 'block' }}>
        {Glyph ? Glyph({ fg, bg: stickerBg }) : <text x="32" y="42" textAnchor="middle" fontSize="22" fontWeight="800" fill={fg} fontFamily="Inter">{s.name[0]}</text>}
      </svg>
      {/* glossy highlight */}
      <div style={{
        position: 'absolute', top: '8%', left: '14%', width: '36%', height: '22%',
        borderRadius: '50%', background: 'rgba(255,255,255,.45)', filter: 'blur(2px)',
      }}/>
    </div>
  );
}

// Deterministic small rotation per sound for hand-stickered feel
function cRotation(seed) {
  return ((seed * 37) % 7) - 3; // -3..+3deg
}

function CTile({ s, theme, dark, density, playing, hover, onHover, fav }) {
  const tone = ctone(s.tone, (C_TONE[s.tone]||C_TONE.amber).light, dark);
  const tHue = (C_TONE[s.tone] || C_TONE.amber).hue;
  const tSat = (C_TONE[s.tone] || C_TONE.amber).sat;
  const tintBg = dark
    ? `hsl(${tHue} ${Math.min(40, tSat)}% 13%)`
    : `hsl(${tHue} ${Math.min(60, tSat)}% 93%)`;
  const tintBgHi = dark
    ? `hsl(${tHue} ${Math.min(50, tSat)}% 17%)`
    : `hsl(${tHue} ${Math.min(70, tSat)}% 89%)`;
  const isPlaying = !!playing;

  const D = density === 'compact'
    ? { tile: 156, pad: 14, name: 13.5, meta: 10, sticker: 48, radius: 16 }
    : density === 'comfy'
    ? { tile: 224, pad: 22, name: 19, meta: 12.5, sticker: 84, radius: 24 }
    : { tile: 192, pad: 18, name: 16, meta: 11.5, sticker: 64, radius: 20 };

  const tileRot = cRotation(s.seed);

  return (
    <div onMouseEnter={() => onHover(s.id)} onMouseLeave={() => onHover(null)} style={{
      position: 'relative', height: D.tile, padding: D.pad,
      background: hover || isPlaying ? tintBgHi : tintBg,
      borderRadius: D.radius, cursor: 'pointer',
      display: 'flex', flexDirection: 'column',
      border: isPlaying ? `2px solid ${tone}` : `1px solid ${theme.hairline}`,
      boxShadow: isPlaying
        ? `0 0 0 4px ${tone}22, 0 14px 30px ${tone}33`
        : hover
        ? `0 8px 18px rgba(0,0,0,${dark ? 0.5 : 0.1})`
        : `0 1px 2px rgba(0,0,0,${dark ? 0.3 : 0.04})`,
      transform: hover ? `translateY(-3px) rotate(${tileRot * 0.6}deg)` : 'none',
      transition: 'transform .18s cubic-bezier(.2,.7,.3,1.2), box-shadow .15s, background .15s',
      overflow: 'hidden',
    }}>
      {/* favorite star (top-right) */}
      {fav && (
        <div style={{ position: 'absolute', top: 10, right: 10, color: '#f59e0b', fontSize: 14, filter: 'drop-shadow(0 1px 2px rgba(0,0,0,.2))' }}>★</div>
      )}

      {/* category label */}
      <div style={{
        fontSize: D.meta, fontWeight: 700,
        color: dark ? `hsl(${tHue} 50% 75%)` : `hsl(${tHue} 60% 30%)`,
        textTransform: 'uppercase', letterSpacing: '.08em',
      }}>{s.cat}</div>

      {/* sticker centered */}
      <div style={{ display: 'flex', justifyContent: 'center', alignItems: 'center', flex: 1, marginTop: 4 }}>
        <CSticker s={s} dark={dark} size={D.sticker} rotation={tileRot * 1.5}/>
      </div>

      {/* name */}
      <div style={{
        fontSize: D.name, fontWeight: 800, color: theme.ink, lineHeight: 1.05,
        letterSpacing: '-0.015em', textWrap: 'pretty', textAlign: 'center',
        display: '-webkit-box', WebkitLineClamp: 2, WebkitBoxOrient: 'vertical', overflow: 'hidden',
      }}>{s.name}</div>

      {/* footer row */}
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginTop: 8 }}>
        <span style={{ fontSize: D.meta, fontWeight: 600, color: theme.inkDim, fontVariantNumeric: 'tabular-nums' }}>{s.dur}</span>
        {s.hk ? (
          <span style={{
            fontSize: D.meta - 0.5, fontWeight: 700, fontFamily: 'ui-monospace, monospace',
            color: dark ? '#fff' : theme.ink,
            background: dark ? 'rgba(255,255,255,.1)' : 'rgba(0,0,0,.08)',
            padding: '2px 6px', borderRadius: 4, transform: 'rotate(-1deg)',
          }}>{s.hk}</span>
        ) : (
          <span style={{
            width: 24, height: 24, borderRadius: 12,
            background: tone, color: '#fff',
            display: 'flex', alignItems: 'center', justifyContent: 'center',
            boxShadow: `0 3px 6px ${tone}55`, transform: 'rotate(-3deg)',
          }}>{isPlaying ? <HHIcon.pause s={11}/> : <HHIcon.play s={11}/>}</span>
        )}
      </div>

      {/* play overlay if has hotkey (since hotkey took the slot) */}
      {s.hk && (hover || isPlaying) && (
        <div style={{
          position: 'absolute', bottom: D.pad, right: D.pad + 50,
          width: 28, height: 28, borderRadius: 14, background: tone, color: '#fff',
          display: 'flex', alignItems: 'center', justifyContent: 'center',
          boxShadow: `0 4px 8px ${tone}55`, transform: 'rotate(-3deg)',
        }}>{isPlaying ? <HHIcon.pause s={12}/> : <HHIcon.play s={12}/>}</div>
      )}
    </div>
  );
}

function CRow({ s, theme, dark, playing, fav }) {
  const tone = ctone(s.tone, (C_TONE[s.tone]||C_TONE.amber).light, dark);
  const tHue = (C_TONE[s.tone] || C_TONE.amber).hue;
  const isPlaying = !!playing;
  return (
    <div style={{
      display: 'grid', gridTemplateColumns: '52px 1fr 60px 70px 32px 24px',
      alignItems: 'center', gap: 14, padding: '10px 16px',
      background: isPlaying ? (dark ? `hsl(${tHue} 30% 13%)` : `hsl(${tHue} 70% 95%)`) : 'transparent',
      borderBottom: `1px solid ${theme.hairline}`,
    }}>
      <CSticker s={s} dark={dark} size={40} rotation={cRotation(s.seed) * 1.2}/>
      <div>
        <div style={{ fontSize: 14, fontWeight: 700, color: theme.ink, letterSpacing: '-.01em' }}>{s.name}</div>
        <div style={{ fontSize: 10.5, color: theme.inkFaint, marginTop: 2, fontWeight: 600, textTransform: 'uppercase', letterSpacing: '.06em' }}>{s.cat}</div>
      </div>
      <div style={{ fontSize: 11.5, color: theme.inkDim, fontVariantNumeric: 'tabular-nums' }}>{s.dur}</div>
      <div>{s.hk && <span style={{
        fontSize: 10.5, fontWeight: 700, fontFamily: 'ui-monospace, monospace',
        background: dark?'rgba(255,255,255,.1)':'rgba(0,0,0,.07)', color: theme.ink,
        padding: '2px 6px', borderRadius: 4,
      }}>{s.hk}</span>}</div>
      <div style={{
        width: 28, height: 28, borderRadius: 14, background: tone, color:'#fff',
        display:'flex', alignItems:'center', justifyContent:'center',
        boxShadow: `0 3px 6px ${tone}55`,
      }}>{isPlaying ? <HHIcon.pause s={12}/> : <HHIcon.play s={12}/>}</div>
      <div style={{ color: fav ? '#f59e0b' : theme.inkFaint, fontSize: 14, textAlign: 'center' }}>{fav ? '★' : '☆'}</div>
    </div>
  );
}

function DirectionC({ dark, density, view, frameW = 1180, frameH = 760 }) {
  const theme = dark ? C_DARK : C_LIGHT;
  const FAVS = ['goose-honk', 'vine-boom', 'fus-ro-dah', 'airhorn', 'bruh', 'wow'];
  const CATS_WITH_FAV = ['Favorites', ...HH_CATEGORIES];
  const [cat, setCat] = React.useState('All');
  const [q, setQ] = React.useState('');
  const [hover, setHover] = React.useState(null);
  const [playing] = React.useState('goose-honk');
  const [progress, setProgress] = React.useState(0.32);
  const [vol, setVol] = React.useState(0.85);

  let filtered = HH_SOUNDS;
  if (cat === 'Favorites') filtered = filtered.filter(s => FAVS.includes(s.id));
  else if (cat !== 'All') filtered = filtered.filter(s => s.cat === cat);
  if (q) filtered = filtered.filter(s => s.name.toLowerCase().includes(q.toLowerCase()));

  React.useEffect(() => {
    let raf, last = performance.now();
    const tick = (t) => { const dt = (t - last) / 1000; last = t; setProgress(p => p + dt * 0.18 > 1 ? 0.05 : p + dt * 0.18); raf = requestAnimationFrame(tick); };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, []);

  const cols = density === 'compact' ? 6 : density === 'comfy' ? 4 : 5;
  const tileGap = density === 'compact' ? 10 : density === 'comfy' ? 18 : 14;

  return (
    <div style={{
      width: frameW, height: frameH, background: theme.bg, color: theme.ink,
      fontFamily: '"Inter", ui-sans-serif, system-ui, sans-serif',
      borderRadius: 16, overflow: 'hidden', display: 'flex', flexDirection: 'column',
      border: `1px solid ${theme.hairline2}`, boxShadow: '0 30px 80px rgba(0,0,0,.22)',
      position: 'relative',
    }}>
      {/* Decorative goose poking from top-left corner */}
      <div style={{ position: 'absolute', top: -12, left: 28, transform: 'rotate(-12deg)', zIndex: 1, pointerEvents: 'none', opacity: dark ? 0.35 : 0.18 }}>
        <svg width="50" height="50" viewBox="0 0 64 64"><GooseGlyph color={theme.accent} accent={theme.paper}/></svg>
      </div>

      {/* Header */}
      <div style={{
        display: 'flex', alignItems: 'center', gap: 16, padding: '18px 24px 14px',
        background: theme.bg, borderBottom: `1px solid ${theme.hairline}`, position: 'relative', zIndex: 2,
      }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
          <div style={{
            width: 48, height: 48, borderRadius: 16,
            background: `conic-gradient(from 200deg at 60% 40%, ${theme.accent}, ${theme.accentDeep}, ${theme.accent})`,
            display: 'flex', alignItems: 'center', justifyContent: 'center',
            boxShadow: `0 6px 16px ${theme.accent}66, inset 0 1px 0 rgba(255,255,255,.5), inset 0 -3px 0 rgba(0,0,0,.15)`,
            transform: 'rotate(-5deg)',
          }}>
            <svg width="32" height="32" viewBox="0 0 64 64"><GooseGlyph color="#1a1208" accent="#fffbeb"/></svg>
          </div>
          <div>
            <div style={{ fontSize: 24, fontWeight: 800, letterSpacing: '-0.025em', lineHeight: 1, fontStyle: 'italic' }}>
              Honk<span style={{ color: theme.accent }}>Honk</span>
            </div>
            <div style={{ fontSize: 11, color: theme.inkDim, marginTop: 4, fontWeight: 500 }}>
              {HH_SOUNDS.length} sounds · 1 mic live · ready to honk 🪿
            </div>
          </div>
        </div>

        <div style={{ flex: 1 }}/>

        <div style={{
          width: 340, display: 'flex', alignItems: 'center', gap: 10,
          height: 42, padding: '0 16px', borderRadius: 999,
          background: theme.panel, border: `1px solid ${theme.hairline2}`,
          boxShadow: 'inset 0 1px 2px rgba(0,0,0,.06)',
        }}>
          <span style={{ color: theme.inkDim }}><HHIcon.search s={16}/></span>
          <input value={q} onChange={e => setQ(e.target.value)} placeholder="Find a sound to honk…"
            style={{ flex: 1, background: 'transparent', border: 'none', outline: 'none', color: theme.ink, fontSize: 13.5, fontFamily: 'inherit' }}/>
        </div>

        <button style={{
          height: 42, padding: '0 18px', borderRadius: 999,
          background: `linear-gradient(140deg, ${theme.accent}, ${theme.accentDeep})`,
          border: 'none', color: '#1a1208', fontSize: 13, fontWeight: 800, fontFamily: 'inherit', cursor: 'pointer',
          display: 'flex', alignItems: 'center', gap: 8, transform: 'rotate(-1deg)',
          boxShadow: `0 4px 12px ${theme.accent}66, inset 0 1px 0 rgba(255,255,255,.5), inset 0 -2px 0 rgba(0,0,0,.15)`,
        }}>
          <HHIcon.stop s={13}/> Stop all
        </button>

        <button style={{
          height: 42, width: 42, borderRadius: 999,
          background: theme.panel, border: `1px solid ${theme.hairline2}`,
          color: theme.inkDim, cursor: 'pointer', display: 'flex',
          alignItems: 'center', justifyContent: 'center',
        }}>
          <HHIcon.cog s={16}/>
        </button>
      </div>

      {/* category chips with Favorites */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '14px 24px 12px', overflowX: 'auto', position: 'relative', zIndex: 2 }}>
        {CATS_WITH_FAV.map(c => {
          const active = c === cat;
          const count = c === 'Favorites' ? FAVS.length : c === 'All' ? HH_SOUNDS.length : HH_SOUNDS.filter(s => s.cat === c).length;
          const rot = ((c.charCodeAt(0) % 5) - 2) * 0.5;
          return (
            <button key={c} onClick={() => setCat(c)} style={{
              height: 36, padding: '0 14px', borderRadius: 999,
              background: active ? theme.ink : theme.panel,
              color: active ? theme.bg : theme.ink,
              border: `1px solid ${active ? theme.ink : theme.hairline}`,
              fontSize: 12.5, fontWeight: 700, fontFamily: 'inherit', cursor: 'pointer',
              display: 'flex', alignItems: 'center', gap: 8, whiteSpace: 'nowrap',
              boxShadow: active ? '0 4px 10px rgba(0,0,0,.18)' : 'none',
              transform: active ? `rotate(${rot}deg)` : 'none',
              transition: 'transform .15s, background .15s',
            }}>
              {c === 'Favorites' && <span style={{ color: active ? theme.accent : '#f59e0b' }}>★</span>}
              {c === 'Honk' && <svg width={14} height={14} viewBox="0 0 64 64"><GooseGlyph color={active ? theme.accent : theme.accent} accent="#fff"/></svg>}
              {c}
              <span style={{
                fontSize: 11, padding: '1px 7px', borderRadius: 999,
                background: active ? 'rgba(255,255,255,.18)' : theme.panelDeep,
                color: active ? theme.bg : theme.inkDim, fontWeight: 700,
                fontVariantNumeric: 'tabular-nums',
              }}>{count}</span>
            </button>
          );
        })}
      </div>

      {/* Grid */}
      <div style={{ flex: 1, padding: '4px 24px 24px', overflow: 'auto', position: 'relative', zIndex: 2 }}>
        {view === 'list' ? (
          <div style={{ background: theme.panel, borderRadius: 14, overflow: 'hidden', border: `1px solid ${theme.hairline}` }}>
            {filtered.map(s => <CRow key={s.id} s={s} theme={theme} dark={dark} playing={s.id===playing} fav={FAVS.includes(s.id)}/>)}
          </div>
        ) : (
          <div style={{ display: 'grid', gridTemplateColumns: `repeat(${cols}, 1fr)`, gap: tileGap }}>
            {filtered.map(s => <CTile key={s.id} s={s} theme={theme} dark={dark} density={density}
              playing={s.id===playing} hover={hover===s.id} onHover={setHover} fav={FAVS.includes(s.id)}/>)}
          </div>
        )}
      </div>

      {/* Bottom now-playing */}
      <div style={{
        display: 'flex', alignItems: 'center', gap: 16, padding: '14px 24px',
        borderTop: `1px solid ${theme.hairline}`, background: theme.panel, position: 'relative', zIndex: 2,
      }}>
        {playing && (() => {
          const sp = HH_SOUNDS.find(s => s.id === playing);
          const tone = ctone(sp.tone, (C_TONE[sp.tone]||C_TONE.amber).light, dark);
          return (
            <>
              <CSticker s={sp} dark={dark} size={44} rotation={-4}/>
              <div>
                <div style={{ fontSize: 14, fontWeight: 800, color: theme.ink, lineHeight: 1.1, letterSpacing: '-.01em' }}>{sp.name}</div>
                <div style={{ fontSize: 10.5, color: theme.inkDim, marginTop: 3, fontWeight: 600, textTransform: 'uppercase', letterSpacing: '.06em' }}>HONKING NOW · {sp.cat}</div>
              </div>
              <div style={{ flex: 1, height: 6, background: theme.panelDeep, borderRadius: 3, position: 'relative', maxWidth: 320 }}>
                <div style={{ position: 'absolute', left: 0, top: 0, bottom: 0, width: `${progress*100}%`, background: tone, borderRadius: 3 }}/>
              </div>
            </>
          );
        })()}
        <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginLeft: 'auto' }}>
          <span style={{ color: theme.inkDim }}><HHIcon.vol s={16}/></span>
          <div style={{ width: 140, height: 6, background: theme.panelDeep, borderRadius: 3, position: 'relative' }}>
            <div style={{ position: 'absolute', left: 0, top: 0, bottom: 0, width: `${vol*100}%`, background: theme.accent, borderRadius: 3 }}/>
            <div style={{ position: 'absolute', left: `calc(${vol*100}% - 8px)`, top: -5, width: 16, height: 16, borderRadius: 16, background: '#fff', boxShadow: '0 2px 6px rgba(0,0,0,.2)' }}/>
          </div>
          <span style={{ fontSize: 12, color: theme.inkDim, fontVariantNumeric: 'tabular-nums', minWidth: 32 }}>{Math.round(vol*100)}%</span>
        </div>
      </div>
    </div>
  );
}

window.DirectionC = DirectionC;
window.CSticker = CSticker;
window.GooseGlyph = GooseGlyph;
window.C_LIGHT = C_LIGHT;
window.C_DARK = C_DARK;
window.C_TONE = C_TONE;
window.cRotation = cRotation;
window.ctone = ctone;
