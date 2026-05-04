// honkhonk-editor.jsx — Per-sound editor in C "Confetti" style
// Modal-style sheet that overlays the main window. Edit one sound: name, color,
// hotkey, favorite, trim (start/end on a waveform).

function PerSoundEditor({ dark, frameW = 1180, frameH = 760 }) {
  const theme = dark ? C_DARK : C_LIGHT;

  // Pretend the user is editing this one
  const sound = HH_SOUNDS.find(s => s.id === 'vine-boom') || HH_SOUNDS[0];
  const [name, setName] = React.useState(sound.name);
  const [tone, setTone] = React.useState(sound.tone);
  const [fav, setFav] = React.useState(true);
  const [hk, setHk] = React.useState(sound.hk || 'F1');
  const [vol, setVol] = React.useState(0.85);
  const [trimL, setTrimL] = React.useState(0.06);
  const [trimR, setTrimR] = React.useState(0.92);

  return (
    <div style={{
      width: frameW, height: frameH, background: theme.bg, color: theme.ink,
      fontFamily: '"Inter", ui-sans-serif, system-ui, sans-serif',
      borderRadius: 16, overflow: 'hidden', display: 'flex',
      border: `1px solid ${theme.hairline2}`, boxShadow: '0 30px 80px rgba(0,0,0,.22)',
      position: 'relative',
    }}>
      {/* dimmed background to imply main window underneath */}
      <div style={{
        position: 'absolute', inset: 0, background: dark ? 'rgba(0,0,0,.55)' : 'rgba(26,20,9,.45)',
        backdropFilter: 'blur(2px)',
      }}/>

      {/* faint hint of grid behind */}
      <div style={{ position: 'absolute', inset: 0, padding: '90px 24px 0', display: 'grid', gridTemplateColumns: 'repeat(5, 1fr)', gap: 14, opacity: 0.18, pointerEvents: 'none' }}>
        {HH_SOUNDS.slice(0, 10).map(s => (
          <div key={s.id} style={{ height: 192, borderRadius: 20, background: theme.panel }}/>
        ))}
      </div>

      {/* sheet */}
      <div style={{
        position: 'relative', margin: 'auto', width: 720, maxHeight: '92%',
        background: theme.bg, borderRadius: 24, overflow: 'hidden',
        boxShadow: '0 40px 100px rgba(0,0,0,.45), 0 4px 0 rgba(0,0,0,.1)',
        border: `1px solid ${theme.hairline2}`, transform: 'rotate(-0.4deg)',
        display: 'flex', flexDirection: 'column',
      }}>
        {/* header with sticker preview */}
        <div style={{
          padding: '24px 28px 20px', display: 'flex', alignItems: 'center', gap: 18,
          background: `linear-gradient(180deg, hsl(${C_TONE[tone].hue} ${C_TONE[tone].sat}% ${dark ? 16 : 92}%), ${theme.bg})`,
          borderBottom: `1px solid ${theme.hairline}`, position: 'relative',
        }}>
          <CSticker s={{ ...sound, tone }} dark={dark} size={72} rotation={-6}/>
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: 11, fontWeight: 800, color: theme.inkDim, textTransform: 'uppercase', letterSpacing: '.1em', marginBottom: 4 }}>Editing sound</div>
            <input value={name} onChange={e => setName(e.target.value)} style={{
              width: '100%', background: 'transparent', border: 'none', outline: 'none',
              fontSize: 26, fontWeight: 800, color: theme.ink, fontFamily: 'inherit',
              letterSpacing: '-.025em', fontStyle: 'italic', padding: 0, marginBottom: 4,
            }}/>
            <div style={{ fontSize: 11.5, color: theme.inkDim, fontFamily: 'ui-monospace, monospace' }}>
              vine-boom.mp3 · 0:01 · 48 kHz · 96 kbps
            </div>
          </div>
          <button style={{
            width: 36, height: 36, borderRadius: 18, background: theme.panel,
            border: `1px solid ${theme.hairline2}`, color: theme.inkDim,
            display: 'flex', alignItems: 'center', justifyContent: 'center', cursor: 'pointer',
          }}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><path d="M18 6L6 18M6 6l12 12"/></svg>
          </button>
        </div>

        {/* body */}
        <div style={{ flex: 1, overflow: 'auto', padding: '6px 28px 18px' }}>
          {/* favorite + color */}
          <ERow label="Sticker color" hint="Tints the tile background and the sticker dot." theme={theme}>
            <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
              {Object.keys(C_TONE).map(t => {
                const c = C_TONE[t];
                const tcolor = `hsl(${c.hue} ${c.sat}% ${dark ? c.light - 5 : c.light}%)`;
                const active = t === tone;
                return (
                  <div key={t} onClick={() => setTone(t)} style={{
                    width: 36, height: 36, borderRadius: 12, cursor: 'pointer',
                    background: `radial-gradient(circle at 30% 25%, hsl(${c.hue} ${c.sat}% ${c.light + 18}%), ${tcolor})`,
                    border: active ? `2.5px solid ${theme.ink}` : `1px solid hsl(${c.hue} ${c.sat}% 35%)`,
                    boxShadow: `inset 0 -2px 4px rgba(0,0,0,.18), inset 0 1px 0 rgba(255,255,255,.4)${active ? `, 0 0 0 3px ${theme.bg}, 0 0 0 5px ${theme.ink}` : ''}`,
                    transform: active ? 'rotate(-3deg) scale(1.05)' : 'none',
                  }}/>
                );
              })}
            </div>
          </ERow>

          <ERow label="Favorite" hint="Pin to the Favorites tab." theme={theme}>
            <button onClick={() => setFav(!fav)} style={{
              height: 36, padding: '0 14px', borderRadius: 999,
              background: fav ? '#fef4a8' : theme.panel,
              border: `1.5px solid ${fav ? '#f59e0b' : theme.hairline2}`,
              color: fav ? '#7a5d1f' : theme.inkDim, fontSize: 12.5, fontWeight: 700,
              cursor: 'pointer', fontFamily: 'inherit',
              display: 'inline-flex', alignItems: 'center', gap: 8,
              transform: fav ? 'rotate(-1deg)' : 'none',
              boxShadow: fav ? '0 4px 10px rgba(245, 158, 11, .35)' : 'none',
            }}>
              <span style={{ fontSize: 14 }}>{fav ? '★' : '☆'}</span>
              {fav ? 'Favorited' : 'Mark as favorite'}
            </button>
          </ERow>

          <ERow label="Global hotkey" hint="Triggers the sound from anywhere — even when HonkHonk isn't focused." theme={theme}>
            <div style={{ display: 'flex', gap: 10, alignItems: 'center' }}>
              <div style={{
                padding: '10px 16px', borderRadius: 10,
                background: dark ? 'rgba(255,255,255,.08)' : 'rgba(0,0,0,.06)',
                fontFamily: 'ui-monospace, monospace', fontSize: 14, fontWeight: 800,
                color: theme.ink, letterSpacing: '.02em',
                border: `1.5px solid ${theme.accent}`, transform: 'rotate(-1deg)',
                boxShadow: `0 4px 10px ${theme.accent}33`,
              }}>{hk}</div>
              <button style={{
                height: 38, padding: '0 14px', borderRadius: 10,
                background: theme.panel, border: `1px solid ${theme.hairline2}`,
                fontSize: 12.5, fontWeight: 700, color: theme.ink, cursor: 'pointer', fontFamily: 'inherit',
              }}>Press a key…</button>
              <button style={{
                height: 38, padding: '0 12px', borderRadius: 10,
                background: 'transparent', border: 'none',
                fontSize: 12, fontWeight: 600, color: theme.inkDim, cursor: 'pointer', fontFamily: 'inherit',
              }}>Clear</button>
            </div>
          </ERow>

          <ERow label="Per-sound volume" hint="Bake a volume offset into this sound. Plays louder/quieter than the master." theme={theme}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 14, maxWidth: 380 }}>
              <div style={{ flex: 1, height: 6, background: theme.panelDeep, borderRadius: 3, position: 'relative' }}>
                <div style={{ position: 'absolute', left: 0, top: 0, bottom: 0, width: `${vol*100}%`, background: theme.accent, borderRadius: 3 }}/>
                <div style={{ position: 'absolute', left: `calc(${vol*100}% - 9px)`, top: -6, width: 18, height: 18, borderRadius: 18, background: '#fff', boxShadow: '0 2px 6px rgba(0,0,0,.25)', border: `2px solid ${theme.accent}` }}/>
              </div>
              <span style={{ fontSize: 12, fontWeight: 700, color: theme.inkDim, minWidth: 36, fontVariantNumeric: 'tabular-nums', textAlign: 'right' }}>{Math.round(vol*100)}%</span>
            </div>
          </ERow>

          <ERow label="Trim" hint="Drag the handles to set start and end points. Silent edges get clipped." theme={theme}>
            <div style={{ width: '100%', maxWidth: 540 }}>
              <TrimWaveform tone={tone} dark={dark} theme={theme} trimL={trimL} trimR={trimR} setTrimL={setTrimL} setTrimR={setTrimR}/>
              <div style={{ display: 'flex', justifyContent: 'space-between', marginTop: 8, fontSize: 11, color: theme.inkDim, fontFamily: 'ui-monospace, monospace' }}>
                <span>start <b style={{ color: theme.ink }}>0:{(trimL*1.42).toFixed(2).slice(2)}</b></span>
                <span>length <b style={{ color: theme.ink }}>0:{((trimR-trimL)*1.42).toFixed(2).slice(2)}</b></span>
                <span>end <b style={{ color: theme.ink }}>0:{(trimR*1.42).toFixed(2).slice(2)}</b></span>
              </div>
            </div>
          </ERow>
        </div>

        {/* footer */}
        <div style={{
          padding: '14px 28px', borderTop: `1px solid ${theme.hairline}`,
          background: theme.panel, display: 'flex', alignItems: 'center', gap: 10,
        }}>
          <button style={{
            height: 38, padding: '0 16px', borderRadius: 10,
            background: 'transparent', border: 'none',
            color: '#dc2626', fontSize: 12.5, fontWeight: 700,
            cursor: 'pointer', fontFamily: 'inherit',
          }}>Delete sound</button>
          <div style={{ flex: 1 }}/>
          <button style={{
            height: 40, padding: '0 16px', borderRadius: 10,
            background: theme.panel, border: `1px solid ${theme.hairline2}`,
            color: theme.ink, fontSize: 13, fontWeight: 700,
            cursor: 'pointer', fontFamily: 'inherit',
          }}>Cancel</button>
          <button style={{
            height: 40, padding: '0 22px', borderRadius: 10,
            background: `linear-gradient(140deg, ${theme.accent}, ${theme.accentDeep})`,
            border: 'none', color: '#1a1208', fontSize: 13.5, fontWeight: 800,
            cursor: 'pointer', fontFamily: 'inherit', transform: 'rotate(-1deg)',
            boxShadow: `0 4px 12px ${theme.accent}55, inset 0 1px 0 rgba(255,255,255,.5), inset 0 -2px 0 rgba(0,0,0,.15)`,
          }}>Save honk</button>
        </div>
      </div>
    </div>
  );
}

function ERow({ label, hint, children, theme }) {
  return (
    <div style={{ padding: '14px 0', borderBottom: `1px solid ${theme.hairline}` }}>
      <div style={{ fontSize: 13, fontWeight: 800, color: theme.ink, letterSpacing: '-.005em', marginBottom: 2 }}>{label}</div>
      {hint && <div style={{ fontSize: 11.5, color: theme.inkDim, marginBottom: 10, lineHeight: 1.5 }}>{hint}</div>}
      <div>{children}</div>
    </div>
  );
}

function TrimWaveform({ tone, dark, theme, trimL, trimR, setTrimL, setTrimR }) {
  const W = 540, H = 70;
  const tColor = `hsl(${C_TONE[tone].hue} ${C_TONE[tone].sat}% ${dark ? C_TONE[tone].light - 5 : C_TONE[tone].light}%)`;
  // simple deterministic waveform
  const bars = 64;
  const data = Array.from({ length: bars }, (_, i) => {
    const t = i / bars;
    return 0.2 + 0.8 * Math.abs(Math.sin(i * 0.7) * Math.cos(i * 0.31) * (1 - Math.abs(t - 0.5) * 0.6));
  });
  const barW = W / bars;
  return (
    <div style={{ position: 'relative', width: W, height: H, background: theme.panelDeep, borderRadius: 10, overflow: 'hidden', border: `1px solid ${theme.hairline}` }}>
      <svg width={W} height={H} style={{ display: 'block' }}>
        {data.map((v, i) => {
          const x = i * barW;
          const inRange = (i / bars) >= trimL && (i / bars) <= trimR;
          const h = v * (H - 12);
          return (
            <rect key={i} x={x + 1} y={(H - h) / 2} width={barW - 2} height={h} rx={1}
              fill={inRange ? tColor : (dark ? '#3a3528' : '#c9bfa9')}/>
          );
        })}
      </svg>
      {/* dim outside range */}
      <div style={{ position: 'absolute', top: 0, bottom: 0, left: 0, width: `${trimL * 100}%`, background: dark ? 'rgba(23,20,16,.65)' : 'rgba(244,239,228,.6)' }}/>
      <div style={{ position: 'absolute', top: 0, bottom: 0, right: 0, width: `${(1 - trimR) * 100}%`, background: dark ? 'rgba(23,20,16,.65)' : 'rgba(244,239,228,.6)' }}/>
      {/* handles */}
      {[
        { p: trimL, key: 'l' },
        { p: trimR, key: 'r' },
      ].map(({ p, key }) => (
        <div key={key} style={{
          position: 'absolute', top: -4, bottom: -4, left: `calc(${p * 100}% - 6px)`, width: 12,
          display: 'flex', alignItems: 'center', justifyContent: 'center', cursor: 'ew-resize',
        }}>
          <div style={{ width: 4, height: '100%', background: theme.accent, borderRadius: 2, boxShadow: `0 2px 8px ${theme.accent}88` }}/>
          <div style={{ position: 'absolute', top: '50%', left: '50%', transform: 'translate(-50%,-50%)', width: 14, height: 22, background: theme.accent, borderRadius: 4, border: '2px solid #1a1208', boxShadow: '0 2px 6px rgba(0,0,0,.3)' }}/>
        </div>
      ))}
    </div>
  );
}

window.PerSoundEditor = PerSoundEditor;
