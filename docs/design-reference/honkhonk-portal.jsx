// honkhonk-portal.jsx — KDE GlobalShortcuts portal flow
// 3-frame storyboard: our pre-empt strip → native KDE dialog → resolved state.

function PortalFlow({ dark, frameW = 1180, frameH = 760 }) {
  const theme = dark ? C_DARK : C_LIGHT;

  // ─── Frame 1: our pre-empt strip in context ──────────────────────────────
  // Shows the editor sheet with the hotkey row about to register a binding.
  function Frame1() {
    return (
      <div style={{ position: 'relative', width: frameW, height: frameH, background: theme.bg, borderRadius: 16, overflow: 'hidden', border: `1px solid ${theme.hairline2}` }}>
        {/* dim app behind */}
        <div style={{ position: 'absolute', inset: 0, padding: '70px 24px 24px', display: 'grid', gridTemplateColumns: 'repeat(5, 1fr)', gap: 14, opacity: 0.35 }}>
          {HH_SOUNDS.slice(0, 10).map(s => {
            const sTone = C_TONE[s.tone];
            const tintBg = dark ? `hsl(${sTone.hue} ${Math.min(40, sTone.sat)}% 13%)` : `hsl(${sTone.hue} ${Math.min(60, sTone.sat)}% 93%)`;
            return <div key={s.id} style={{ height: 168, background: tintBg, borderRadius: 18, border: `1px solid ${theme.hairline}` }}/>;
          })}
        </div>
        <div style={{ position: 'absolute', inset: 0, background: dark ? 'rgba(0,0,0,.4)' : 'rgba(26,20,9,.25)' }}/>

        {/* tag */}
        <div style={{ position: 'absolute', top: 18, left: 24, fontSize: 11, fontWeight: 800, color: theme.inkDim, letterSpacing: '.06em', textTransform: 'uppercase', background: theme.panel, padding: '4px 10px', borderRadius: 6, border: `1px solid ${theme.hairline2}` }}>
          1 · Pre-empt strip — our window
        </div>

        {/* Edit sheet, focused on the hotkey row */}
        <div style={{
          position: 'absolute', left: '50%', top: '50%', transform: 'translate(-50%, -50%) rotate(-1deg)',
          width: 580, background: theme.panel, borderRadius: 18, padding: 24,
          border: `1px solid ${theme.hairline2}`, boxShadow: '0 30px 70px rgba(0,0,0,.4)',
        }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 20 }}>
            <CSticker s={HH_SOUNDS.find(s => s.id === 'fus-ro-dah')} dark={dark} size={48} rotation={-4}/>
            <div>
              <div style={{ fontSize: 17, fontWeight: 800, fontStyle: 'italic', letterSpacing: '-.015em' }}>FUS RO DAH</div>
              <div style={{ fontSize: 10.5, color: theme.inkDim, fontWeight: 700, textTransform: 'uppercase', letterSpacing: '.06em', marginTop: 2 }}>Edit · 0:02</div>
            </div>
          </div>

          <div style={{ fontSize: 11, fontWeight: 800, color: theme.inkDim, letterSpacing: '.08em', marginBottom: 8 }}>GLOBAL HOTKEY</div>

          {/* Pre-empt strip */}
          <div style={{
            padding: '12px 14px', borderRadius: 12,
            background: dark ? 'rgba(59, 130, 246, 0.12)' : 'rgba(59, 130, 246, 0.08)',
            border: `1px solid ${dark ? 'rgba(59, 130, 246, 0.4)' : 'rgba(59, 130, 246, 0.3)'}`,
            display: 'flex', alignItems: 'flex-start', gap: 10, marginBottom: 12,
          }}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke={dark ? '#60a5fa' : '#2563eb'} strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" style={{ flexShrink: 0, marginTop: 1 }}>
              <circle cx="12" cy="12" r="9"/>
              <path d="M12 8v4M12 16v.01"/>
            </svg>
            <div style={{ fontSize: 12, lineHeight: 1.55, color: dark ? '#bfdbfe' : '#1e40af' }}>
              <b>Heads up:</b> KDE will ask for permission to register global shortcuts. You'll see one system dialog per app — accept it once and HonkHonk can listen for hotkeys even when the window is hidden.
            </div>
          </div>

          {/* Capture row */}
          <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
            <div style={{
              flex: 1, padding: '14px 16px', borderRadius: 10,
              background: dark ? 'rgba(255,255,255,.05)' : 'rgba(0,0,0,.04)',
              border: `2px dashed ${theme.accent}`, textAlign: 'center',
              fontSize: 13, color: theme.inkDim, fontWeight: 600,
              fontFamily: 'ui-monospace, monospace',
            }}>
              <span style={{ display: 'inline-flex', alignItems: 'center', gap: 8 }}>
                <span style={{ width: 8, height: 8, borderRadius: 8, background: theme.accent, animation: 'none' }}/>
                Press a key combo…
              </span>
            </div>
            <button style={{
              height: 48, padding: '0 14px', borderRadius: 10,
              background: theme.bg, border: `1px solid ${theme.hairline2}`,
              fontSize: 12, fontWeight: 700, color: theme.inkDim, cursor: 'pointer', fontFamily: 'inherit',
            }}>Cancel</button>
          </div>

          <div style={{ fontSize: 10.5, color: theme.inkFaint, fontWeight: 600, marginTop: 8, letterSpacing: '.04em' }}>
            ⇒ pressing now triggers the system permission dialog (frame 2).
          </div>
        </div>
      </div>
    );
  }

  // ─── Frame 2: native KDE portal dialog ───────────────────────────────────
  function Frame2() {
    // Native Breeze styling — same palette as the tray menu.
    const isDark = dark;
    const T = isDark
      ? { winBg: '#2a2e32', winBorder: '#3d4145', text: '#eff0f1', textDim: '#a1a9b1', sep: '#3d4145', accent: '#3daee9', deskBg: '#1d3357', btnBg: '#31363b', btnBorder: '#4d5359' }
      : { winBg: '#fcfcfc', winBorder: '#bdc3c7', text: '#232629', textDim: '#7f8c8d', sep: '#e0e2e4', accent: '#3daee9', deskBg: '#1d3357', btnBg: '#fdfdfd', btnBorder: '#bdc3c7' };

    return (
      <div style={{
        position: 'relative', width: frameW, height: frameH, borderRadius: 16, overflow: 'hidden',
        background: T.deskBg,
        backgroundImage: 'radial-gradient(circle at 30% 40%, rgba(61,174,233,.25), transparent 60%), radial-gradient(circle at 70% 60%, rgba(0,0,0,.4), transparent 70%)',
        border: `1px solid rgba(0,0,0,.2)`,
      }}>
        <div style={{ position: 'absolute', top: 18, left: 24, fontSize: 11, fontWeight: 700, color: 'rgba(255,255,255,.85)', letterSpacing: '.06em', textTransform: 'uppercase', background: 'rgba(0,0,0,.35)', padding: '4px 10px', borderRadius: 6 }}>
          2 · Native KDE dialog — xdg-desktop-portal-kde
        </div>

        {/* Faint app window behind */}
        <div style={{
          position: 'absolute', left: 80, top: 72, width: frameW - 160, height: frameH - 144,
          background: theme.bg, borderRadius: 12, opacity: 0.35,
          border: `1px solid ${theme.hairline2}`, boxShadow: '0 20px 50px rgba(0,0,0,.4)',
        }}/>

        {/* The portal dialog */}
        <div style={{
          position: 'absolute', left: '50%', top: '50%', transform: 'translate(-50%, -50%)',
          width: 480, background: T.winBg, color: T.text,
          border: `1px solid ${T.winBorder}`, borderRadius: 4,
          fontFamily: 'system-ui, "Noto Sans", "Inter", sans-serif',
          boxShadow: '0 24px 60px rgba(0,0,0,.5)', overflow: 'hidden',
        }}>
          {/* titlebar */}
          <div style={{
            height: 30, background: isDark ? '#31363b' : '#eff0f1',
            borderBottom: `1px solid ${T.winBorder}`,
            display: 'flex', alignItems: 'center', padding: '0 8px', gap: 6,
          }}>
            <div style={{ display: 'flex', gap: 4, marginRight: 'auto' }}>
              {[0,1,2].map(i => (
                <div key={i} style={{ width: 10, height: 10, borderRadius: 10, background: ['#27c93f','#fdbc40','#ff5f56'][i] }}/>
              ))}
            </div>
            <span style={{ fontSize: 11.5, color: T.textDim, fontWeight: 600 }}>Allow Global Shortcuts?</span>
            <div style={{ marginLeft: 'auto', display: 'flex', gap: 4, color: T.textDim }}>
              {['_','□','✕'].map((g, i) => (
                <button key={i} style={{ width: 22, height: 22, background: 'transparent', border: 'none', color: 'inherit', fontSize: 11, cursor: 'pointer', fontFamily: 'inherit' }}>{g}</button>
              ))}
            </div>
          </div>

          <div style={{ padding: '20px 22px 16px' }}>
            {/* App identity */}
            <div style={{ display: 'flex', alignItems: 'center', gap: 14, marginBottom: 18 }}>
              <div style={{
                width: 48, height: 48, borderRadius: 12, flexShrink: 0,
                background: 'conic-gradient(from 200deg at 60% 40%, #f59e0b, #b45309, #f59e0b)',
                display: 'flex', alignItems: 'center', justifyContent: 'center',
                border: '1px solid rgba(0,0,0,.15)',
              }}>
                <svg width="32" height="32" viewBox="0 0 64 64"><GooseGlyph color="#1a1208" accent="#fffbeb"/></svg>
              </div>
              <div>
                <div style={{ fontSize: 15, fontWeight: 700 }}>HonkHonk</div>
                <div style={{ fontSize: 11.5, color: T.textDim, marginTop: 2, fontFamily: 'ui-monospace, monospace' }}>
                  org.honkhonk.HonkHonk · Flatpak
                </div>
              </div>
            </div>

            {/* Body */}
            <div style={{ fontSize: 13, lineHeight: 1.55, marginBottom: 16 }}>
              <b>HonkHonk</b> wants to register global keyboard shortcuts. The application will be able to read keyboard input system-wide, even when its window is not in focus.
            </div>

            {/* Permission scope */}
            <div style={{
              padding: '10px 12px', background: isDark ? 'rgba(255,255,255,.04)' : 'rgba(0,0,0,.03)',
              border: `1px solid ${T.sep}`, borderRadius: 4, marginBottom: 18,
              fontSize: 12, lineHeight: 1.5,
            }}>
              <div style={{ fontWeight: 700, marginBottom: 4 }}>Requested shortcuts:</div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 4, color: T.textDim }}>
                <div style={{ display: 'flex', justifyContent: 'space-between' }}><span>Stop all sounds</span><code style={{ fontFamily: 'ui-monospace, monospace', background: isDark ? 'rgba(255,255,255,.06)' : 'rgba(0,0,0,.05)', padding: '1px 5px', borderRadius: 3 }}>Ctrl+Shift+Esc</code></div>
                <div style={{ display: 'flex', justifyContent: 'space-between' }}><span>Trigger sound</span><code style={{ fontFamily: 'ui-monospace, monospace', background: isDark ? 'rgba(255,255,255,.06)' : 'rgba(0,0,0,.05)', padding: '1px 5px', borderRadius: 3 }}>up to 20 user-defined</code></div>
              </div>
            </div>

            {/* "Remember" checkbox */}
            <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 12.5, marginBottom: 18, cursor: 'pointer' }}>
              <span style={{ width: 14, height: 14, borderRadius: 2, background: T.accent, border: `1px solid ${T.accent}`, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                <svg width="9" height="9" viewBox="0 0 24 24" fill="none" stroke="#fff" strokeWidth="3.5" strokeLinecap="round" strokeLinejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
              </span>
              Remember this decision
            </label>

            {/* Buttons */}
            <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end' }}>
              <button style={{
                height: 30, padding: '0 16px', background: T.btnBg, color: T.text,
                border: `1px solid ${T.btnBorder}`, borderRadius: 3, fontSize: 12.5, cursor: 'pointer',
                fontFamily: 'inherit',
              }}>Deny</button>
              <button style={{
                height: 30, padding: '0 16px', background: T.accent, color: '#fff',
                border: `1px solid ${T.accent}`, borderRadius: 3, fontSize: 12.5, fontWeight: 600, cursor: 'pointer',
                fontFamily: 'inherit', boxShadow: '0 0 0 2px rgba(61,174,233,.25)',
              }}>Allow</button>
            </div>
          </div>
        </div>

        <div style={{ position: 'absolute', bottom: 18, left: 24, fontSize: 11, color: 'rgba(255,255,255,.7)', maxWidth: 420, lineHeight: 1.5 }}>
          We don't draw this — KDE does. Our job is to make the moment <i>before</i> the dialog (frame 1) and <i>after</i> it (frame 3) feel intentional.
        </div>
      </div>
    );
  }

  // ─── Frame 3: resolved state — both branches ─────────────────────────────
  function Frame3() {
    return (
      <div style={{ position: 'relative', width: frameW, height: frameH, background: theme.bg, borderRadius: 16, overflow: 'hidden', border: `1px solid ${theme.hairline2}`, padding: '52px 24px 24px' }}>
        <div style={{ position: 'absolute', top: 18, left: 24, fontSize: 11, fontWeight: 800, color: theme.inkDim, letterSpacing: '.06em', textTransform: 'uppercase', background: theme.panel, padding: '4px 10px', borderRadius: 6, border: `1px solid ${theme.hairline2}` }}>
          3 · Resolved — both branches side by side
        </div>

        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 18, height: '100%' }}>
          {/* Allowed branch */}
          <div style={{
            position: 'relative', padding: 26, borderRadius: 16,
            background: theme.panel, border: `2px solid #16a34a`, transform: 'rotate(-1deg)',
            boxShadow: '0 12px 30px rgba(22, 163, 74, .15)',
          }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 14 }}>
              <span style={{ width: 28, height: 28, borderRadius: 28, background: '#16a34a', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#fff" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
              </span>
              <div style={{ fontSize: 16, fontWeight: 800, letterSpacing: '-.01em', fontStyle: 'italic' }}>Allowed</div>
              <span style={{ marginLeft: 'auto', fontSize: 10, fontWeight: 700, color: '#15803d', background: '#dcfce7', padding: '3px 8px', borderRadius: 999, letterSpacing: '.06em' }}>HAPPY PATH</span>
            </div>

            <div style={{ fontSize: 13, lineHeight: 1.55, color: theme.ink, marginBottom: 16 }}>
              We drop a single confetti toast in the top-right of the main window: <i>"Hotkeys are live — try Ctrl+Shift+Esc to stop everything."</i> Auto-dismisses in 8s.
            </div>

            {/* Toast preview */}
            <div style={{
              padding: '12px 14px', borderRadius: 12,
              background: 'linear-gradient(140deg, #fbbf24, #f59e0b)',
              color: '#1a1208', display: 'flex', alignItems: 'center', gap: 10,
              boxShadow: '0 8px 18px rgba(245, 158, 11, .35)', transform: 'rotate(0.8deg)', marginBottom: 14,
            }}>
              <CSticker s={HH_SOUNDS.find(s => s.id === 'goose-honk')} dark={false} size={36} rotation={-6}/>
              <div style={{ flex: 1 }}>
                <div style={{ fontSize: 12, fontWeight: 800, letterSpacing: '-.005em' }}>Hotkeys are live</div>
                <div style={{ fontSize: 11, fontWeight: 600, opacity: 0.85, marginTop: 1 }}>Try <code style={{ background: 'rgba(0,0,0,.12)', padding: '0 4px', borderRadius: 3, fontFamily: 'ui-monospace, monospace' }}>Ctrl+Shift+Esc</code> to stop everything.</div>
              </div>
              <button style={{ width: 22, height: 22, borderRadius: 6, background: 'rgba(0,0,0,.1)', border: 'none', color: 'inherit', cursor: 'pointer' }}>✕</button>
            </div>

            <div style={{ fontSize: 11.5, color: theme.inkDim, lineHeight: 1.55 }}>
              Settings → Hotkeys now shows <b style={{ color: theme.ink }}>portal: registered</b> with a live green dot. The user can keep binding without re-prompting.
            </div>
          </div>

          {/* Denied branch */}
          <div style={{
            position: 'relative', padding: 26, borderRadius: 16,
            background: theme.panel, border: `2px solid #dc2626`, transform: 'rotate(1deg)',
            boxShadow: '0 12px 30px rgba(220, 38, 38, .15)',
          }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 14 }}>
              <span style={{ width: 28, height: 28, borderRadius: 28, background: '#dc2626', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#fff" strokeWidth="3" strokeLinecap="round"><path d="M18 6L6 18M6 6l12 12"/></svg>
              </span>
              <div style={{ fontSize: 16, fontWeight: 800, letterSpacing: '-.01em', fontStyle: 'italic' }}>Denied</div>
              <span style={{ marginLeft: 'auto', fontSize: 10, fontWeight: 700, color: '#991b1b', background: '#fee2e2', padding: '3px 8px', borderRadius: 999, letterSpacing: '.06em' }}>FALLBACK</span>
            </div>

            <div style={{ fontSize: 13, lineHeight: 1.55, color: theme.ink, marginBottom: 16 }}>
              Soft red banner in the editor, NOT a modal. Sound still plays via the in-app button — only global shortcuts are blocked.
            </div>

            {/* Inline banner preview */}
            <div style={{
              padding: '12px 14px', borderRadius: 10,
              background: dark ? 'rgba(220, 38, 38, 0.12)' : 'rgba(220, 38, 38, 0.08)',
              border: `1px solid rgba(220, 38, 38, 0.4)`,
              fontSize: 12, lineHeight: 1.5, color: dark ? '#fca5a5' : '#7f1d1d',
              marginBottom: 14,
            }}>
              <div style={{ display: 'flex', alignItems: 'flex-start', gap: 8, marginBottom: 8 }}>
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" style={{ flexShrink: 0, marginTop: 1 }}>
                  <circle cx="12" cy="12" r="9"/>
                  <path d="M12 9v4M12 17v.01"/>
                </svg>
                <div><b>Hotkey skipped — KDE permission denied.</b> You can still trigger sounds from the window.</div>
              </div>
              <div style={{ display: 'flex', gap: 6, paddingLeft: 22 }}>
                <button style={{ height: 26, padding: '0 10px', borderRadius: 6, background: 'transparent', border: '1px solid currentColor', color: 'inherit', fontSize: 11, fontWeight: 700, cursor: 'pointer', fontFamily: 'inherit' }}>Try again</button>
                <button style={{ height: 26, padding: '0 10px', borderRadius: 6, background: 'transparent', border: 'none', color: 'inherit', fontSize: 11, fontWeight: 600, cursor: 'pointer', fontFamily: 'inherit', opacity: 0.7 }}>How to fix in KDE Settings →</button>
              </div>
            </div>

            <div style={{ fontSize: 11.5, color: theme.inkDim, lineHeight: 1.55 }}>
              Settings → Hotkeys shows <b style={{ color: '#dc2626' }}>portal: denied</b> with a "Re-request permission" button at the top. We never re-prompt automatically.
            </div>
          </div>
        </div>
      </div>
    );
  }

  // Render all 3 stacked vertically
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 24 }}>
      <Frame1/>
      <Frame2/>
      <Frame3/>
    </div>
  );
}

window.PortalFlow = PortalFlow;
