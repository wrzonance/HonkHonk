// honkhonk-tray.jsx — Native-feeling KDE tray menu

function TrayMenu({ dark, frameW = 1180, frameH = 760 }) {
  // Native KDE Plasma styling — Breeze-ish.
  const isDark = dark;
  const T = isDark
    ? { menuBg: '#2a2e32', menuBorder: '#3d4145', text: '#eff0f1', textDim: '#a1a9b1', sep: '#3d4145', hover: '#3daee9', hoverText: '#fff', accent: '#3daee9', deskBg: '#1d3357' }
    : { menuBg: '#fcfcfc', menuBorder: '#bdc3c7', text: '#232629', textDim: '#7f8c8d', sep: '#e0e2e4', hover: '#3daee9', hoverText: '#fff', accent: '#3daee9', deskBg: '#1d3357' };

  // Recent sounds
  const RECENT = ['goose-honk', 'vine-boom', 'airhorn', 'bruh', 'fus-ro-dah'].map(id => HH_SOUNDS.find(s => s.id === id)).filter(Boolean);

  const Item = ({ icon, label, shortcut, hover, sub, danger, sticker }) => (
    <div style={{
      display: 'flex', alignItems: 'center', gap: 10,
      padding: '6px 12px 6px 10px', minHeight: 26,
      background: hover ? T.hover : 'transparent',
      color: hover ? T.hoverText : (danger ? '#da4453' : T.text),
      fontSize: 13, cursor: 'pointer', position: 'relative',
    }}>
      <span style={{ width: 18, display: 'flex', justifyContent: 'center', color: hover ? T.hoverText : T.textDim }}>
        {sticker ? sticker : icon}
      </span>
      <span style={{ flex: 1 }}>{label}</span>
      {sub && <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><path d="M9 6l6 6-6 6"/></svg>}
      {shortcut && <span style={{ fontSize: 11, color: hover ? 'rgba(255,255,255,.85)' : T.textDim, fontFamily: 'ui-monospace, monospace' }}>{shortcut}</span>}
    </div>
  );

  const Sep = () => <div style={{ height: 1, background: T.sep, margin: '4px 0' }}/>;

  return (
    <div style={{
      width: frameW, height: frameH, background: T.deskBg,
      borderRadius: 16, overflow: 'hidden', position: 'relative',
      backgroundImage: 'radial-gradient(circle at 30% 40%, rgba(61,174,233,.25), transparent 60%), radial-gradient(circle at 70% 60%, rgba(0,0,0,.4), transparent 70%)',
      fontFamily: '"Inter", ui-sans-serif, system-ui, sans-serif',
      border: `1px solid rgba(0,0,0,.2)`, boxShadow: '0 30px 80px rgba(0,0,0,.3)',
    }}>
      {/* faint plasma desktop hint */}
      <div style={{
        position: 'absolute', top: 24, left: 32, color: 'rgba(255,255,255,.5)',
        fontSize: 11.5, fontWeight: 600, letterSpacing: '.06em', textTransform: 'uppercase',
      }}>KDE Plasma · System tray</div>

      {/* taskbar at bottom */}
      <div style={{
        position: 'absolute', left: 0, right: 0, bottom: 0, height: 44,
        background: 'rgba(36, 38, 41, 0.92)', borderTop: '1px solid rgba(255,255,255,.08)',
        display: 'flex', alignItems: 'center', padding: '0 12px', backdropFilter: 'blur(8px)',
      }}>
        <button style={{ height: 32, width: 32, borderRadius: 4, background: 'rgba(255,255,255,.08)', border: 'none', display: 'flex', alignItems: 'center', justifyContent: 'center', color: '#fff', cursor: 'pointer', marginRight: 6 }}>
          <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><path d="M12 2L2 12h3v8h6v-6h2v6h6v-8h3z"/></svg>
        </button>
        <div style={{ flex: 1 }}/>
        {/* tray icons */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '0 10px', height: 32, borderRadius: 4, background: 'rgba(255,255,255,.04)' }}>
          {/* HonkHonk tray icon (highlighted, the menu is anchored to this) */}
          <div style={{
            width: 26, height: 26, borderRadius: 6, position: 'relative',
            background: 'rgba(61,174,233,.25)', border: '1px solid rgba(61,174,233,.5)',
            display: 'flex', alignItems: 'center', justifyContent: 'center',
          }}>
            <svg width="20" height="20" viewBox="0 0 64 64"><GooseGlyph color="#fbbf24" accent="#1d3357"/></svg>
            {/* live dot */}
            <span style={{ position: 'absolute', top: -2, right: -2, width: 8, height: 8, borderRadius: 8, background: '#27ae60', border: '1.5px solid #242629' }}/>
          </div>
          {[
            <svg key="1" width="16" height="16" viewBox="0 0 24 24" fill="rgba(255,255,255,.7)"><path d="M3 9v6h4l5 5V4L7 9z"/></svg>,
            <svg key="2" width="16" height="16" viewBox="0 0 24 24" fill="rgba(255,255,255,.7)"><path d="M2 6h20v3H2zm0 5h20v3H2zm0 5h20v3H2z"/></svg>,
            <svg key="3" width="16" height="16" viewBox="0 0 24 24" fill="rgba(255,255,255,.7)"><circle cx="12" cy="12" r="9"/></svg>,
            <svg key="4" width="16" height="16" viewBox="0 0 24 24" fill="rgba(255,255,255,.7)"><path d="M2 6h20v12H2z"/></svg>,
          ].map((s, i) => (
            <div key={i} style={{ width: 22, height: 22, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>{s}</div>
          ))}
          <span style={{ color: 'rgba(255,255,255,.85)', fontSize: 12, fontWeight: 600, fontVariantNumeric: 'tabular-nums', marginLeft: 6, fontFamily: 'ui-monospace, monospace' }}>13:42</span>
        </div>
      </div>

      {/* Menu — anchored above the HonkHonk tray icon */}
      <div style={{
        position: 'absolute', right: 92, bottom: 56, width: 280,
        background: T.menuBg, color: T.text,
        border: `1px solid ${T.menuBorder}`,
        borderRadius: 4, padding: '4px 0',
        boxShadow: '0 12px 32px rgba(0,0,0,.5), 0 2px 4px rgba(0,0,0,.3)',
        overflow: 'hidden',
      }}>
        {/* Header showing app */}
        <div style={{
          padding: '8px 12px 10px', display: 'flex', alignItems: 'center', gap: 10,
          background: isDark ? 'rgba(255,255,255,.03)' : 'rgba(0,0,0,.02)',
          borderBottom: `1px solid ${T.sep}`,
        }}>
          <div style={{
            width: 30, height: 30, borderRadius: 8,
            background: 'conic-gradient(from 200deg at 60% 40%, #f59e0b, #b45309, #f59e0b)',
            display: 'flex', alignItems: 'center', justifyContent: 'center',
          }}>
            <svg width="20" height="20" viewBox="0 0 64 64"><GooseGlyph color="#1a1208" accent="#fffbeb"/></svg>
          </div>
          <div>
            <div style={{ fontSize: 13, fontWeight: 700, letterSpacing: '-.005em' }}>HonkHonk</div>
            <div style={{ fontSize: 10.5, color: T.textDim, marginTop: 1, display: 'flex', alignItems: 'center', gap: 5 }}>
              <span style={{ width: 6, height: 6, borderRadius: 6, background: '#27ae60' }}/>
              <span>Mic live · Goose Honk playing</span>
            </div>
          </div>
        </div>

        <Item icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="5" width="4" height="14" rx="1"/><rect x="14" y="5" width="4" height="14" rx="1"/></svg>} label="Stop all sounds" shortcut="Ctrl+⇧+Esc" danger/>
        <Sep/>

        <div style={{ padding: '4px 12px 2px', fontSize: 10.5, color: T.textDim, fontWeight: 700, letterSpacing: '.05em', textTransform: 'uppercase' }}>Recently played</div>
        {RECENT.map(s => (
          <Item key={s.id} sticker={<CSticker s={s} dark={dark} size={18} rotation={cRotation(s.seed)}/>} label={s.name} shortcut={s.hk}/>
        ))}
        <Sep/>

        <Item icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="9"/><path d="M8 12l3 3 5-6"/></svg>} label="Show HonkHonk window"/>
        <Item icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M3 12h18"/></svg>} label="Hide window"/>
        <Item icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><rect x="2" y="6" width="20" height="12" rx="2"/><path d="M7 10h.01M12 10h.01M17 10h.01"/></svg>} label="Slot manager…" sub/>
        <Item icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9c.36.79.92 1.36 1.51 1H21a2 2 0 0 1 0 4h-.09c-.79.04-1.36.26-1.51 1z"/></svg>} label="Settings…"/>
        <Sep/>

        <Item icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4M16 17l5-5-5-5M21 12H9"/></svg>} label="Quit HonkHonk" shortcut="Ctrl+Q" hover/>
      </div>
    </div>
  );
}

window.TrayMenu = TrayMenu;
