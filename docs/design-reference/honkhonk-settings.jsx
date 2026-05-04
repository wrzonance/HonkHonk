// honkhonk-settings.jsx — Settings panel in C "Confetti" style
// Full-window swap. Five sections: Audio · Library · Hotkeys · Appearance · About.

function SettingsRow({ label, hint, children, theme }) {
  return (
    <div style={{
      display: 'grid', gridTemplateColumns: '260px 1fr', gap: 24, padding: '18px 0',
      borderBottom: `1px solid ${theme.hairline}`, alignItems: 'start',
    }}>
      <div>
        <div style={{ fontSize: 13.5, fontWeight: 700, color: theme.ink, letterSpacing: '-.005em' }}>{label}</div>
        {hint && <div style={{ fontSize: 11.5, color: theme.inkDim, marginTop: 4, lineHeight: 1.5 }}>{hint}</div>}
      </div>
      <div>{children}</div>
    </div>
  );
}

function SToggle({ value, theme, dark, onLabel = 'On', offLabel = 'Off' }) {
  return (
    <div style={{
      display: 'inline-flex', alignItems: 'center', gap: 10,
      padding: '6px 14px 6px 6px', borderRadius: 999,
      background: value ? theme.accent : theme.panelDeep,
      border: `1px solid ${value ? theme.accentDeep : theme.hairline2}`,
      cursor: 'pointer', transform: value ? 'rotate(-1deg)' : 'none',
      boxShadow: value ? `0 4px 10px ${theme.accent}55, inset 0 -2px 0 rgba(0,0,0,.15), inset 0 1px 0 rgba(255,255,255,.4)` : 'none',
      transition: 'background .15s, transform .15s',
    }}>
      <div style={{
        width: 22, height: 22, borderRadius: 11,
        background: value ? '#fffbeb' : (dark ? '#3a3528' : '#fff'),
        boxShadow: '0 2px 4px rgba(0,0,0,.2)',
        transform: value ? 'translateX(0)' : 'translateX(0)',
      }}/>
      <span style={{ fontSize: 12, fontWeight: 800, color: value ? '#1a1208' : theme.inkDim, letterSpacing: '.02em' }}>
        {value ? onLabel : offLabel}
      </span>
    </div>
  );
}

function SSelect({ value, options, theme, dark }) {
  return (
    <div style={{
      display: 'inline-flex', alignItems: 'center', gap: 10,
      height: 38, padding: '0 14px', borderRadius: 10,
      background: theme.panel, border: `1px solid ${theme.hairline2}`,
      fontSize: 13, fontWeight: 600, color: theme.ink, cursor: 'pointer',
      minWidth: 280, justifyContent: 'space-between',
      boxShadow: 'inset 0 1px 0 rgba(255,255,255,.5)',
    }}>
      <span>{value}</span>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" style={{ color: theme.inkDim }}>
        <path d="M6 9l6 6 6-6"/>
      </svg>
    </div>
  );
}

function SSlider({ value, theme, label }) {
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 14, maxWidth: 380 }}>
      <div style={{ flex: 1, height: 6, background: theme.panelDeep, borderRadius: 3, position: 'relative' }}>
        <div style={{ position: 'absolute', left: 0, top: 0, bottom: 0, width: `${value*100}%`, background: theme.accent, borderRadius: 3 }}/>
        <div style={{ position: 'absolute', left: `calc(${value*100}% - 9px)`, top: -6, width: 18, height: 18, borderRadius: 18, background: '#fff', boxShadow: '0 2px 6px rgba(0,0,0,.25), inset 0 1px 0 rgba(255,255,255,.6)', border: `2px solid ${theme.accent}` }}/>
      </div>
      <span style={{ fontSize: 12, fontWeight: 700, color: theme.inkDim, minWidth: 36, fontVariantNumeric: 'tabular-nums', textAlign: 'right' }}>{label}</span>
    </div>
  );
}

function SRadio({ value, options, theme }) {
  return (
    <div style={{ display: 'inline-flex', gap: 4, padding: 4, borderRadius: 12, background: theme.panelDeep, border: `1px solid ${theme.hairline}` }}>
      {options.map(o => {
        const active = o === value;
        return (
          <div key={o} style={{
            padding: '7px 14px', borderRadius: 8, fontSize: 12.5, fontWeight: 700,
            background: active ? theme.panel : 'transparent',
            color: active ? theme.ink : theme.inkDim,
            boxShadow: active ? '0 1px 3px rgba(0,0,0,.1)' : 'none',
            cursor: 'pointer', textTransform: 'capitalize',
          }}>{o}</div>
        );
      })}
    </div>
  );
}

function SettingsPanel({ dark, density, view, frameW = 1180, frameH = 760 }) {
  const theme = dark ? C_DARK : C_LIGHT;
  const [active, setActive] = React.useState('audio');

  const NAV = [
    { id: 'audio',      label: 'Audio',      glyph: <HHIcon.vol s={16}/> },
    { id: 'library',    label: 'Library',    glyph: <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M3 7l2-2h6l2 2h8a1 1 0 0 1 1 1v11a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V8a1 1 0 0 1 1-1z"/></svg> },
    { id: 'hotkeys',    label: 'Hotkeys',    glyph: <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><rect x="2" y="6" width="20" height="12" rx="2"/><path d="M7 10h.01M12 10h.01M17 10h.01M7 14h10"/></svg> },
    { id: 'appearance', label: 'Appearance', glyph: <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="9"/><path d="M12 3a9 9 0 0 1 0 18 4.5 4.5 0 0 1 0-9 4.5 4.5 0 0 0 0-9z"/></svg> },
    { id: 'about',      label: 'About',      glyph: <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="9"/><path d="M12 8v.01M11 12h1v4h1"/></svg> },
  ];

  return (
    <div style={{
      width: frameW, height: frameH, background: theme.bg, color: theme.ink,
      fontFamily: '"Inter", ui-sans-serif, system-ui, sans-serif',
      borderRadius: 16, overflow: 'hidden', display: 'flex', flexDirection: 'column',
      border: `1px solid ${theme.hairline2}`, boxShadow: '0 30px 80px rgba(0,0,0,.22)',
      position: 'relative',
    }}>
      {/* Header */}
      <div style={{
        display: 'flex', alignItems: 'center', gap: 16, padding: '16px 24px',
        borderBottom: `1px solid ${theme.hairline}`, background: theme.bg,
      }}>
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
          <div style={{ fontSize: 22, fontWeight: 800, letterSpacing: '-0.025em', fontStyle: 'italic' }}>Settings</div>
          <div style={{ fontSize: 12, color: theme.inkDim, fontWeight: 500 }}>· tweak the honk</div>
        </div>
        <div style={{ flex: 1 }}/>
        <div style={{ transform: 'rotate(8deg)', opacity: 0.7 }}>
          <svg width="42" height="42" viewBox="0 0 64 64"><GooseGlyph color={theme.accent} accent={theme.paper}/></svg>
        </div>
      </div>

      {/* Body */}
      <div style={{ flex: 1, display: 'flex', overflow: 'hidden' }}>
        {/* Sidebar */}
        <div style={{
          width: 220, padding: '18px 14px', borderRight: `1px solid ${theme.hairline}`,
          background: theme.panel, display: 'flex', flexDirection: 'column', gap: 4,
        }}>
          {NAV.map(n => {
            const isActive = n.id === active;
            return (
              <div key={n.id} onClick={() => setActive(n.id)} style={{
                display: 'flex', alignItems: 'center', gap: 12,
                padding: '10px 14px', borderRadius: 10, cursor: 'pointer',
                background: isActive ? theme.ink : 'transparent',
                color: isActive ? theme.bg : theme.ink,
                fontSize: 13.5, fontWeight: 700, letterSpacing: '-.005em',
                transform: isActive ? 'rotate(-1deg)' : 'none',
                boxShadow: isActive ? '0 4px 10px rgba(0,0,0,.18)' : 'none',
                transition: 'background .15s, transform .15s',
              }}>
                <span style={{ color: isActive ? theme.accent : theme.inkDim }}>{n.glyph}</span>
                {n.label}
              </div>
            );
          })}
          <div style={{ flex: 1 }}/>
          <div style={{
            padding: '12px 14px', borderRadius: 10, background: theme.panelDeep,
            fontSize: 11.5, color: theme.inkDim, lineHeight: 1.5,
          }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 4 }}>
              <span style={{ width: 7, height: 7, borderRadius: 7, background: theme.good, display: 'inline-block', boxShadow: `0 0 6px ${theme.good}` }}/>
              <b style={{ color: theme.ink, fontSize: 11.5 }}>Audio engine running</b>
            </div>
            <span style={{ fontFamily: 'ui-monospace, monospace', fontSize: 10.5 }}>cpal · 48kHz · 256f</span>
          </div>
        </div>

        {/* Content */}
        <div style={{ flex: 1, padding: '24px 36px 32px', overflow: 'auto' }}>
          {active === 'audio' && (
            <div>
              <SectionHeader theme={theme} title="Audio" sub="Where HonkHonk listens, where it speaks, and what powers it."/>
              <SettingsRow theme={theme} label="Virtual microphone" hint="Creates a fake mic other apps (Discord, Zoom, OBS) can pick up. Backed by PipeWire on KDE.">
                <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
                  <SToggle value={true} theme={theme} dark={dark} onLabel="HonkHonk Mic · live" offLabel="Off"/>
                  <div style={{ fontSize: 11, color: theme.inkFaint, fontFamily: 'ui-monospace, monospace' }}>node.name = <span style={{ color: theme.inkDim }}>honkhonk-virtual-source</span></div>
                </div>
              </SettingsRow>
              <SettingsRow theme={theme} label="Monitor output" hint="Where you (and only you) hear the sound. Usually your headphones.">
                <SSelect theme={theme} dark={dark} value="Built-in Audio Analog Stereo · alsa_output.pci-0000_00_1f.3"/>
              </SettingsRow>
              <SettingsRow theme={theme} label="Mic passthrough" hint="Pipe your real mic into the virtual one so people still hear you. Off = sound effects only mode.">
                <div style={{ display: 'flex', flexDirection: 'column', gap: 14, maxWidth: 380 }}>
                  <SToggle value={true} theme={theme} dark={dark} onLabel="Passing through" offLabel="Off"/>
                  <SSlider value={0.78} theme={theme} label="78%"/>
                  <div style={{ fontSize: 11, color: theme.inkFaint }}>Source: <span style={{ color: theme.inkDim, fontFamily: 'ui-monospace, monospace' }}>HD Webcam C920 · alsa_input.usb-046d_C920</span></div>
                </div>
              </SettingsRow>
              <SettingsRow theme={theme} label="Sample rate" hint="Match your virtual mic's rate. 48kHz is the safe default on Linux audio stacks.">
                <SRadio theme={theme} value="48 kHz" options={['44.1 kHz', '48 kHz', '96 kHz']}/>
              </SettingsRow>
              <SettingsRow theme={theme} label="Renderer" hint="GPU-accelerated wgpu is faster; tiny-skia is the CPU fallback for old hardware or VMs.">
                <SRadio theme={theme} value="wgpu" options={['wgpu', 'tiny-skia']}/>
              </SettingsRow>
            </div>
          )}

          {active === 'library' && (
            <div>
              <SectionHeader theme={theme} title="Library" sub="Where HonkHonk looks for your sounds."/>
              <SettingsRow theme={theme} label="Sound folders" hint="HonkHonk watches these folders. Drop in MP3 / WAV / OGG / FLAC and they appear instantly.">
                <div style={{ display: 'flex', flexDirection: 'column', gap: 8, maxWidth: 540 }}>
                  {['~/Sounds/honkhonk', '~/Music/memes', '~/Downloads/airhorns'].map((p, i) => (
                    <div key={p} style={{
                      display: 'flex', alignItems: 'center', gap: 10, padding: '10px 12px',
                      background: theme.panel, borderRadius: 10, border: `1px solid ${theme.hairline}`,
                    }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke={theme.accent} strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M3 7l2-2h6l2 2h8a1 1 0 0 1 1 1v11a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V8a1 1 0 0 1 1-1z"/></svg>
                      <span style={{ fontFamily: 'ui-monospace, monospace', fontSize: 12.5, color: theme.ink, flex: 1 }}>{p}</span>
                      <span style={{ fontSize: 11, color: theme.inkDim, fontVariantNumeric: 'tabular-nums' }}>{[14, 6, 4][i]} sounds</span>
                      <button style={{ background: 'none', border: 'none', color: theme.inkFaint, cursor: 'pointer', padding: 4 }}>
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="M18 6L6 18M6 6l12 12"/></svg>
                      </button>
                    </div>
                  ))}
                  <button style={{
                    height: 36, borderRadius: 10, background: 'transparent',
                    border: `1.5px dashed ${theme.hairline2}`, color: theme.inkDim,
                    fontSize: 12.5, fontWeight: 700, cursor: 'pointer', fontFamily: 'inherit',
                    display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 8,
                  }}>+ Add a folder</button>
                </div>
              </SettingsRow>
              <SettingsRow theme={theme} label="Scan now" hint="Force a re-scan. Normally automatic via inotify.">
                <button style={{
                  height: 38, padding: '0 18px', borderRadius: 10,
                  background: theme.panel, border: `1px solid ${theme.hairline2}`,
                  fontSize: 13, fontWeight: 700, color: theme.ink, cursor: 'pointer', fontFamily: 'inherit',
                  display: 'inline-flex', alignItems: 'center', gap: 8,
                }}>
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round"><path d="M3 12a9 9 0 0 1 15-6.7L21 8M21 3v5h-5M21 12a9 9 0 0 1-15 6.7L3 16M3 21v-5h5"/></svg>
                  Re-scan now
                </button>
              </SettingsRow>
              <SettingsRow theme={theme} label="Supported formats" hint="HonkHonk uses Symphonia for decoding — these are the formats it can read.">
                <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6 }}>
                  {['MP3', 'WAV', 'OGG Vorbis', 'FLAC', 'AAC', 'Opus'].map(f => (
                    <span key={f} style={{
                      padding: '5px 11px', borderRadius: 999, background: theme.panel,
                      border: `1px solid ${theme.hairline2}`, fontSize: 11.5, fontWeight: 700,
                      color: theme.inkDim, fontFamily: 'ui-monospace, monospace',
                    }}>{f}</span>
                  ))}
                </div>
              </SettingsRow>
            </div>
          )}

          {active === 'hotkeys' && (
            <div>
              <SectionHeader theme={theme} title="Hotkeys" sub="Global shortcuts that work even when HonkHonk isn't focused."/>
              <SettingsRow theme={theme} label="KDE Global Shortcuts" hint="HonkHonk registers via the org.kde.kglobalaccel D-Bus portal. KDE owns the binding UI from there.">
                <div style={{ display: 'flex', flexDirection: 'column', gap: 10, maxWidth: 540 }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '10px 14px', background: `${theme.good}15`, border: `1px solid ${theme.good}55`, borderRadius: 10 }}>
                    <span style={{ width: 8, height: 8, borderRadius: 8, background: theme.good }}/>
                    <span style={{ fontSize: 12.5, fontWeight: 700, color: dark ? theme.good : '#0a6e2e' }}>Connected to KDE portal</span>
                    <span style={{ marginLeft: 'auto', fontSize: 11, color: theme.inkDim, fontFamily: 'ui-monospace, monospace' }}>kglobalaccel5 · 5.27.10</span>
                  </div>
                  <button style={{
                    height: 38, padding: '0 16px', borderRadius: 10,
                    background: theme.panel, border: `1px solid ${theme.hairline2}`,
                    fontSize: 12.5, fontWeight: 700, color: theme.ink, cursor: 'pointer', fontFamily: 'inherit',
                    alignSelf: 'flex-start',
                  }}>Open KDE shortcut settings →</button>
                </div>
              </SettingsRow>
              <SettingsRow theme={theme} label="Quick bindings" hint="Top sounds with global hotkeys. Edit per-sound to change.">
                <div style={{ display: 'flex', flexDirection: 'column', gap: 6, maxWidth: 540 }}>
                  {[
                    { name: 'Vine Boom', tone: 'amber', hk: 'F1' },
                    { name: 'Bruh', tone: 'orange', hk: 'F2' },
                    { name: 'Goose Honk', tone: 'amber', hk: 'F3' },
                    { name: 'Airhorn', tone: 'red', hk: 'F4' },
                    { name: 'Fus Ro Dah', tone: 'sky', hk: 'Ctrl+Shift+Y' },
                  ].map(s => (
                    <div key={s.name} style={{
                      display: 'flex', alignItems: 'center', gap: 12, padding: '8px 12px',
                      background: theme.panel, borderRadius: 8, border: `1px solid ${theme.hairline}`,
                    }}>
                      <CSticker s={{ id: 'pop', tone: s.tone, name: s.name, seed: s.name.length }} dark={dark} size={28} rotation={-3}/>
                      <span style={{ fontSize: 13, fontWeight: 700, color: theme.ink, flex: 1 }}>{s.name}</span>
                      <span style={{
                        fontSize: 11.5, fontWeight: 700, fontFamily: 'ui-monospace, monospace',
                        background: dark ? 'rgba(255,255,255,.08)' : 'rgba(0,0,0,.06)', color: theme.ink,
                        padding: '4px 10px', borderRadius: 6,
                      }}>{s.hk}</span>
                    </div>
                  ))}
                </div>
              </SettingsRow>
              <SettingsRow theme={theme} label="Stop-all shortcut" hint="Panic button — silences everything currently playing.">
                <span style={{
                  fontSize: 12.5, fontWeight: 800, fontFamily: 'ui-monospace, monospace',
                  background: dark ? 'rgba(255,255,255,.1)' : 'rgba(0,0,0,.08)', color: theme.ink,
                  padding: '6px 12px', borderRadius: 8, transform: 'rotate(-1deg)', display: 'inline-block',
                }}>Ctrl + Shift + Esc</span>
              </SettingsRow>
            </div>
          )}

          {active === 'appearance' && (
            <div>
              <SectionHeader theme={theme} title="Appearance" sub="How honky should HonkHonk look today?"/>
              <SettingsRow theme={theme} label="Theme" hint="Light leans paper-and-sticker; dark is the same energy at night.">
                <SRadio theme={theme} value={dark ? 'Dark' : 'Light'} options={['Light', 'Dark', 'System']}/>
              </SettingsRow>
              <SettingsRow theme={theme} label="Density" hint="Compact fits more on screen; comfy makes the stickers bigger.">
                <SRadio theme={theme} value={density} options={['compact', 'regular', 'comfy']}/>
              </SettingsRow>
              <SettingsRow theme={theme} label="View" hint="Grid for browsing, list for hunting.">
                <SRadio theme={theme} value={view} options={['grid', 'list']}/>
              </SettingsRow>
              <SettingsRow theme={theme} label="Accent intensity" hint="How loud the goose-yellow gets across the UI. Subtle is OBS-friendly; full is full carnival.">
                <SRadio theme={theme} value="full" options={['subtle', 'medium', 'full']}/>
              </SettingsRow>
              <SettingsRow theme={theme} label="Wonkiness" hint="Per-tile rotation jitter. 0 = pixel-aligned; 100 = sticker book.">
                <SSlider value={0.7} theme={theme} label="70%"/>
              </SettingsRow>
            </div>
          )}

          {active === 'about' && (
            <div>
              <SectionHeader theme={theme} title="About" sub="The bird is the word."/>
              <div style={{
                display: 'flex', alignItems: 'center', gap: 20, padding: '20px 0',
                borderBottom: `1px solid ${theme.hairline}`,
              }}>
                <div style={{
                  width: 84, height: 84, borderRadius: 28,
                  background: `conic-gradient(from 200deg at 60% 40%, ${theme.accent}, ${theme.accentDeep}, ${theme.accent})`,
                  display: 'flex', alignItems: 'center', justifyContent: 'center',
                  boxShadow: `0 8px 24px ${theme.accent}55, inset 0 2px 0 rgba(255,255,255,.5), inset 0 -4px 0 rgba(0,0,0,.15)`,
                  transform: 'rotate(-6deg)',
                }}>
                  <svg width="56" height="56" viewBox="0 0 64 64"><GooseGlyph color="#1a1208" accent="#fffbeb"/></svg>
                </div>
                <div>
                  <div style={{ fontSize: 28, fontWeight: 800, fontStyle: 'italic', letterSpacing: '-.025em' }}>
                    Honk<span style={{ color: theme.accent }}>Honk</span>
                  </div>
                  <div style={{ fontSize: 13, color: theme.inkDim, marginTop: 4, fontWeight: 500 }}>v0.1.0 · "First Honk" · Iced 0.13</div>
                  <div style={{ fontSize: 12, color: theme.inkFaint, marginTop: 6, lineHeight: 1.5, maxWidth: 460 }}>
                    A soundboard for KDE. Built with Rust, Iced, cpal, and Symphonia. Made because the world needs more goose noises.
                  </div>
                </div>
              </div>
              <SettingsRow theme={theme} label="License" hint="">
                <span style={{ fontSize: 12.5, fontFamily: 'ui-monospace, monospace', color: theme.ink, padding: '4px 10px', background: theme.panel, borderRadius: 6, border: `1px solid ${theme.hairline}` }}>GPL-3.0-or-later</span>
              </SettingsRow>
              <SettingsRow theme={theme} label="Credits" hint="">
                <div style={{ fontSize: 12.5, color: theme.inkDim, lineHeight: 1.7 }}>
                  Iced · <span style={{ color: theme.ink }}>iced-rs</span><br/>
                  cpal · <span style={{ color: theme.ink }}>RustAudio</span><br/>
                  Symphonia · <span style={{ color: theme.ink }}>pdeljanov</span><br/>
                  Inter · <span style={{ color: theme.ink }}>Rasmus Andersson</span>
                </div>
              </SettingsRow>
              <SettingsRow theme={theme} label="Source" hint="">
                <button style={{
                  height: 38, padding: '0 16px', borderRadius: 10,
                  background: theme.panel, border: `1px solid ${theme.hairline2}`,
                  fontSize: 13, fontWeight: 700, color: theme.ink, cursor: 'pointer', fontFamily: 'inherit',
                }}>github.com/you/honkhonk →</button>
              </SettingsRow>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function SectionHeader({ theme, title, sub }) {
  return (
    <div style={{ marginBottom: 8, paddingBottom: 14, borderBottom: `2px solid ${theme.ink}` }}>
      <div style={{ fontSize: 26, fontWeight: 800, letterSpacing: '-.03em', fontStyle: 'italic' }}>{title}</div>
      {sub && <div style={{ fontSize: 13, color: theme.inkDim, marginTop: 4, fontWeight: 500 }}>{sub}</div>}
    </div>
  );
}

window.SettingsPanel = SettingsPanel;
