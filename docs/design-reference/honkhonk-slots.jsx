// honkhonk-slots.jsx — Phase 2 slot manager in C "Confetti" style
// 20 fixed slots arranged as 4 rows of 5 (stream-deck-ish).
// Each slot can be empty, bound to a sound, with optional KDE global shortcut.

function SlotManager({ dark, frameW = 1180, frameH = 760 }) {
  const theme = dark ? C_DARK : C_LIGHT;

  // 20 slots — some bound, some empty
  const SLOTS = [
    { i: 1,  bound: 'goose-honk',   hk: 'Meta+1' },
    { i: 2,  bound: 'goose-honk-2', hk: 'Meta+2' },
    { i: 3,  bound: 'goose-flock',  hk: 'Meta+3' },
    { i: 4,  bound: 'vine-boom',    hk: 'F1' },
    { i: 5,  bound: 'airhorn',      hk: 'F4' },
    { i: 6,  bound: 'bruh',         hk: 'F2' },
    { i: 7,  bound: 'oof',          hk: null },
    { i: 8,  bound: 'sad-violin',   hk: null },
    { i: 9,  bound: 'wow',          hk: 'Meta+9' },
    { i: 10, bound: 'fus-ro-dah',   hk: 'Ctrl+Shift+Y', conflict: false },
    { i: 11, bound: 'rickroll',     hk: 'Meta+R' },
    { i: 12, bound: 'all-star',     hk: null },
    { i: 13, bound: 'metal-pipe',   hk: 'Meta+P', conflict: true },
    { i: 14, bound: 'bonk',         hk: null },
    { i: 15, bound: 'pop',          hk: null },
    { i: 16, bound: 'whoosh',       hk: null },
    { i: 17, bound: null, hk: null },
    { i: 18, bound: null, hk: null },
    { i: 19, bound: null, hk: null },
    { i: 20, bound: null, hk: null },
  ];

  const [selected, setSelected] = React.useState(13); // shows the binding panel for slot 13

  return (
    <div style={{
      width: frameW, height: frameH, background: theme.bg, color: theme.ink,
      fontFamily: '"Inter", ui-sans-serif, system-ui, sans-serif',
      borderRadius: 16, overflow: 'hidden', display: 'flex', flexDirection: 'column',
      border: `1px solid ${theme.hairline2}`, boxShadow: '0 30px 80px rgba(0,0,0,.22)',
    }}>
      {/* Header */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 16, padding: '16px 24px', borderBottom: `1px solid ${theme.hairline}` }}>
        <button style={{
          height: 38, padding: '0 14px 0 10px', borderRadius: 10,
          background: theme.panel, border: `1px solid ${theme.hairline2}`,
          fontSize: 12.5, fontWeight: 700, color: theme.ink, cursor: 'pointer',
          display: 'flex', alignItems: 'center', gap: 6, fontFamily: 'inherit',
        }}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><path d="M15 18l-6-6 6-6"/></svg>
          Back to sounds
        </button>
        <div style={{ display: 'flex', alignItems: 'baseline', gap: 10 }}>
          <div style={{ fontSize: 22, fontWeight: 800, letterSpacing: '-0.025em', fontStyle: 'italic' }}>Slots</div>
          <div style={{ fontSize: 12, color: theme.inkDim, fontWeight: 500 }}>· 20 fixed slots, drag a sound to bind</div>
        </div>
        <div style={{ flex: 1 }}/>
        <div style={{ display: 'flex', alignItems: 'center', gap: 12, fontSize: 11.5, color: theme.inkDim, fontWeight: 600 }}>
          <span><b style={{ color: theme.ink }}>16</b> bound</span>
          <span style={{ opacity: 0.4 }}>·</span>
          <span><b style={{ color: theme.ink }}>10</b> with hotkey</span>
          <span style={{ opacity: 0.4 }}>·</span>
          <span><b style={{ color: '#dc2626' }}>1</b> conflict</span>
        </div>
      </div>

      <div style={{ flex: 1, display: 'flex', overflow: 'hidden' }}>
        {/* Slot board */}
        <div style={{ flex: 1, padding: '24px 28px', overflow: 'auto' }}>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
            {[0, 1, 2, 3].map(rowIdx => (
              <div key={rowIdx} style={{ display: 'flex', alignItems: 'center', gap: 14 }}>
                <div style={{ width: 28, fontSize: 11, fontWeight: 800, color: theme.inkFaint, letterSpacing: '.1em', textAlign: 'right' }}>
                  ROW{rowIdx + 1}
                </div>
                <div style={{ display: 'grid', gridTemplateColumns: 'repeat(5, 1fr)', gap: 12, flex: 1 }}>
                  {SLOTS.slice(rowIdx * 5, rowIdx * 5 + 5).map(slot => {
                    const sound = slot.bound ? HH_SOUNDS.find(s => s.id === slot.bound) : null;
                    const isSelected = slot.i === selected;
                    const tone = sound ? C_TONE[sound.tone] : null;
                    const tColor = sound ? `hsl(${tone.hue} ${tone.sat}% ${dark ? tone.light - 5 : tone.light}%)` : null;
                    const tintBg = sound
                      ? (dark ? `hsl(${tone.hue} ${Math.min(40, tone.sat)}% 13%)` : `hsl(${tone.hue} ${Math.min(60, tone.sat)}% 93%)`)
                      : theme.panel;
                    const rot = ((slot.i * 37) % 7) - 3;
                    return (
                      <div key={slot.i} onClick={() => setSelected(slot.i)} style={{
                        position: 'relative', height: 138, padding: 12, borderRadius: 18, cursor: 'pointer',
                        background: tintBg,
                        border: isSelected ? `2.5px solid ${theme.ink}` : (sound ? `1px solid ${theme.hairline}` : `2px dashed ${theme.hairline2}`),
                        boxShadow: isSelected ? `0 8px 20px rgba(0,0,0,.18), 0 0 0 4px ${theme.accent}33` : (sound ? `0 1px 2px rgba(0,0,0,${dark ? 0.3 : 0.04})` : 'none'),
                        transform: isSelected ? `rotate(${rot * 0.5}deg) translateY(-2px)` : 'none',
                        display: 'flex', flexDirection: 'column', overflow: 'hidden',
                      }}>
                        {/* slot number */}
                        <div style={{
                          position: 'absolute', top: 8, left: 10,
                          fontSize: 10, fontWeight: 800, color: theme.inkFaint,
                          fontFamily: 'ui-monospace, monospace', letterSpacing: '.05em',
                        }}>#{String(slot.i).padStart(2, '0')}</div>

                        {/* conflict badge */}
                        {slot.conflict && (
                          <div style={{
                            position: 'absolute', top: 8, right: 8,
                            fontSize: 9, fontWeight: 800, color: '#fff', background: '#dc2626',
                            padding: '2px 6px', borderRadius: 999, letterSpacing: '.06em',
                          }}>CONFLICT</div>
                        )}

                        {sound ? (
                          <>
                            <div style={{ display: 'flex', justifyContent: 'center', flex: 1, alignItems: 'center', marginTop: 6 }}>
                              <CSticker s={sound} dark={dark} size={48} rotation={rot * 1.4}/>
                            </div>
                            <div style={{
                              fontSize: 11.5, fontWeight: 800, color: theme.ink, lineHeight: 1.1,
                              textAlign: 'center', textWrap: 'pretty', letterSpacing: '-.005em',
                              display: '-webkit-box', WebkitLineClamp: 1, WebkitBoxOrient: 'vertical', overflow: 'hidden',
                            }}>{sound.name}</div>
                            <div style={{ display: 'flex', justifyContent: 'center', marginTop: 6 }}>
                              {slot.hk ? (
                                <span style={{
                                  fontSize: 10, fontWeight: 800, fontFamily: 'ui-monospace, monospace',
                                  color: slot.conflict ? '#fff' : theme.ink,
                                  background: slot.conflict ? '#dc2626' : (dark ? 'rgba(255,255,255,.1)' : 'rgba(0,0,0,.07)'),
                                  padding: '2px 7px', borderRadius: 4, transform: 'rotate(-1deg)',
                                }}>{slot.hk}</span>
                              ) : (
                                <span style={{
                                  fontSize: 10, fontWeight: 700, color: theme.inkFaint,
                                  border: `1px dashed ${theme.hairline2}`, padding: '2px 7px', borderRadius: 4,
                                }}>no hotkey</span>
                              )}
                            </div>
                          </>
                        ) : (
                          <div style={{ flex: 1, display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', gap: 6, color: theme.inkFaint }}>
                            <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" style={{ opacity: 0.6 }}>
                              <path d="M12 5v14M5 12h14"/>
                            </svg>
                            <div style={{ fontSize: 10.5, fontWeight: 700, letterSpacing: '.06em' }}>EMPTY</div>
                          </div>
                        )}
                      </div>
                    );
                  })}
                </div>
              </div>
            ))}
          </div>

          {/* footer hint */}
          <div style={{ marginTop: 18, padding: '10px 14px', background: theme.panel, borderRadius: 10, border: `1px solid ${theme.hairline}`, fontSize: 11.5, color: theme.inkDim, lineHeight: 1.5, display: 'flex', alignItems: 'center', gap: 10 }}>
            <span style={{ fontSize: 14 }}>🪿</span>
            <span><b style={{ color: theme.ink }}>Pro tip:</b> drag any sound from the main grid onto a slot to bind it. Slots stay in place — even if you delete the bound sound, the slot stays #13.</span>
          </div>
        </div>

        {/* Binding side panel */}
        <div style={{ width: 320, borderLeft: `1px solid ${theme.hairline}`, background: theme.panel, padding: '22px 22px 18px', overflow: 'auto' }}>
          {(() => {
            const slot = SLOTS.find(s => s.i === selected);
            const sound = slot && slot.bound ? HH_SOUNDS.find(s => s.id === slot.bound) : null;
            return (
              <>
                <div style={{ fontSize: 10.5, fontWeight: 800, color: theme.inkDim, letterSpacing: '.1em', marginBottom: 10 }}>
                  SLOT #{String(slot.i).padStart(2, '0')}
                </div>
                {sound ? (
                  <>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 18 }}>
                      <CSticker s={sound} dark={dark} size={56} rotation={-5}/>
                      <div>
                        <div style={{ fontSize: 17, fontWeight: 800, letterSpacing: '-.015em', fontStyle: 'italic' }}>{sound.name}</div>
                        <div style={{ fontSize: 11, color: theme.inkDim, marginTop: 2, fontWeight: 600, textTransform: 'uppercase', letterSpacing: '.06em' }}>{sound.cat} · {sound.dur}</div>
                      </div>
                    </div>

                    {slot.conflict && (
                      <div style={{ padding: '10px 12px', background: 'rgba(220, 38, 38, 0.1)', border: '1px solid rgba(220, 38, 38, 0.4)', borderRadius: 10, fontSize: 11.5, color: dark ? '#fca5a5' : '#7f1d1d', marginBottom: 16, lineHeight: 1.5, display: 'flex', alignItems: 'flex-start', gap: 8 }}>
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" style={{ flexShrink: 0, marginTop: 1 }}>
                          <path d="M12 9v4M12 17v.01"/>
                          <circle cx="12" cy="12" r="9"/>
                        </svg>
                        <span><b>Hotkey conflict:</b> KDE has <code style={{ fontFamily: 'ui-monospace, monospace', background: 'rgba(0,0,0,.1)', padding: '1px 4px', borderRadius: 3 }}>Meta+P</code> bound to "Print Screen." Pick another or override in KDE settings.</span>
                      </div>
                    )}

                    <div style={{ fontSize: 11, fontWeight: 800, color: theme.inkDim, letterSpacing: '.08em', marginBottom: 8 }}>GLOBAL HOTKEY</div>
                    <div style={{ display: 'flex', gap: 8, marginBottom: 14, alignItems: 'center' }}>
                      <div style={{
                        flex: 1, padding: '10px 14px', borderRadius: 10,
                        background: dark ? 'rgba(255,255,255,.08)' : 'rgba(0,0,0,.06)',
                        fontFamily: 'ui-monospace, monospace', fontSize: 13.5, fontWeight: 800,
                        color: theme.ink, textAlign: 'center',
                        border: `1.5px solid ${slot.conflict ? '#dc2626' : theme.accent}`,
                        boxShadow: `0 4px 10px ${slot.conflict ? '#dc262633' : theme.accent + '33'}`,
                      }}>{slot.hk || '—'}</div>
                      <button style={{
                        height: 40, padding: '0 12px', borderRadius: 10,
                        background: theme.bg, border: `1px solid ${theme.hairline2}`,
                        fontSize: 11.5, fontWeight: 700, color: theme.ink, cursor: 'pointer', fontFamily: 'inherit',
                      }}>Rebind…</button>
                    </div>

                    <div style={{ fontSize: 11, fontWeight: 800, color: theme.inkDim, letterSpacing: '.08em', marginBottom: 8 }}>KDE PORTAL STATUS</div>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '8px 12px', background: theme.bg, border: `1px solid ${theme.hairline}`, borderRadius: 8, fontSize: 11.5, color: theme.inkDim, marginBottom: 18 }}>
                      <span style={{ width: 8, height: 8, borderRadius: 8, background: theme.good, boxShadow: `0 0 6px ${theme.good}` }}/>
                      <span><b style={{ color: theme.ink }}>Registered</b> via kglobalaccel</span>
                    </div>

                    <div style={{ display: 'flex', gap: 8 }}>
                      <button style={{
                        flex: 1, height: 38, borderRadius: 10,
                        background: theme.bg, border: `1px solid ${theme.hairline2}`,
                        fontSize: 12.5, fontWeight: 700, color: theme.ink, cursor: 'pointer', fontFamily: 'inherit',
                      }}>Edit sound</button>
                      <button style={{
                        flex: 1, height: 38, borderRadius: 10,
                        background: 'transparent', border: `1px solid rgba(220,38,38,.4)`,
                        fontSize: 12.5, fontWeight: 700, color: '#dc2626', cursor: 'pointer', fontFamily: 'inherit',
                      }}>Unbind</button>
                    </div>
                  </>
                ) : (
                  <>
                    <div style={{
                      padding: '32px 16px', textAlign: 'center', borderRadius: 14,
                      background: theme.bg, border: `2px dashed ${theme.hairline2}`, marginBottom: 14,
                    }}>
                      <div style={{ fontSize: 32, marginBottom: 8, opacity: 0.4 }}>🪿</div>
                      <div style={{ fontSize: 13, fontWeight: 700, color: theme.ink, marginBottom: 6 }}>Slot is empty</div>
                      <div style={{ fontSize: 11.5, color: theme.inkDim, lineHeight: 1.5 }}>Drag a sound from the main grid, or pick one below.</div>
                    </div>
                    <button style={{
                      width: '100%', height: 40, borderRadius: 10,
                      background: `linear-gradient(140deg, ${theme.accent}, ${theme.accentDeep})`,
                      border: 'none', color: '#1a1208', fontSize: 13, fontWeight: 800,
                      cursor: 'pointer', fontFamily: 'inherit', transform: 'rotate(-1deg)',
                      boxShadow: `0 4px 12px ${theme.accent}55`,
                    }}>Pick a sound…</button>
                  </>
                )}
              </>
            );
          })()}
        </div>
      </div>
    </div>
  );
}

window.SlotManager = SlotManager;
