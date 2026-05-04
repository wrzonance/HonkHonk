// honkhonk-context.jsx — Right-click context menu on a tile (C style)

function ContextMenu({ dark, frameW = 1180, frameH = 760 }) {
  const theme = dark ? C_DARK : C_LIGHT;
  const sound = HH_SOUNDS.find(s => s.id === 'fus-ro-dah') || HH_SOUNDS[0];
  const tone = C_TONE[sound.tone];

  // Position the context menu where the right-click happened
  const anchorX = 540, anchorY = 320;

  const Item = ({ icon, label, shortcut, sub, danger, hover, sticker }) => (
    <div style={{
      display: 'flex', alignItems: 'center', gap: 10,
      padding: '8px 14px 8px 12px', minHeight: 30,
      background: hover ? theme.ink : 'transparent',
      color: hover ? theme.bg : (danger ? '#dc2626' : theme.ink),
      fontSize: 13, fontWeight: 600, cursor: 'pointer', position: 'relative',
      borderRadius: 8, margin: '0 4px',
    }}>
      <span style={{ width: 18, display: 'flex', justifyContent: 'center', color: hover ? theme.accent : theme.inkDim }}>
        {sticker ? sticker : icon}
      </span>
      <span style={{ flex: 1, letterSpacing: '-.005em' }}>{label}</span>
      {sub && <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><path d="M9 6l6 6-6 6"/></svg>}
      {shortcut && (
        <span style={{
          fontSize: 10.5, fontWeight: 700, fontFamily: 'ui-monospace, monospace',
          color: hover ? 'rgba(251,243,223,.7)' : theme.inkFaint,
          background: hover ? 'rgba(255,255,255,.08)' : (dark ? 'rgba(255,255,255,.05)' : 'rgba(0,0,0,.05)'),
          padding: '2px 6px', borderRadius: 4,
        }}>{shortcut}</span>
      )}
    </div>
  );

  const Sep = () => <div style={{ height: 1, background: theme.hairline, margin: '4px 12px' }}/>;

  return (
    <div style={{
      width: frameW, height: frameH, background: theme.bg, color: theme.ink,
      fontFamily: '"Inter", ui-sans-serif, system-ui, sans-serif',
      borderRadius: 16, overflow: 'hidden', position: 'relative',
      border: `1px solid ${theme.hairline2}`, boxShadow: '0 30px 80px rgba(0,0,0,.22)',
    }}>
      {/* Faint hint of the main grid behind, dimmed */}
      <div style={{ position: 'absolute', inset: 0, padding: '90px 24px 24px', display: 'grid', gridTemplateColumns: 'repeat(5, 1fr)', gap: 14, opacity: 0.55 }}>
        {HH_SOUNDS.slice(0, 15).map((s, i) => {
          const isTarget = s.id === sound.id;
          const sTone = C_TONE[s.tone];
          const tintBg = dark
            ? `hsl(${sTone.hue} ${Math.min(40, sTone.sat)}% 13%)`
            : `hsl(${sTone.hue} ${Math.min(60, sTone.sat)}% 93%)`;
          const rot = ((s.seed * 37) % 7) - 3;
          return (
            <div key={s.id} style={{
              height: 192, padding: 18, borderRadius: 20,
              background: tintBg,
              border: isTarget ? `2.5px solid ${theme.accent}` : `1px solid ${theme.hairline}`,
              transform: isTarget ? `rotate(${rot * 0.6}deg) translateY(-3px)` : 'none',
              boxShadow: isTarget ? `0 0 0 4px ${theme.accent}33, 0 14px 30px rgba(0,0,0,.15)` : 'none',
              display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 8, overflow: 'hidden',
            }}>
              <div style={{ fontSize: 11, fontWeight: 700, color: theme.inkDim, alignSelf: 'flex-start', textTransform: 'uppercase', letterSpacing: '.08em' }}>{s.cat}</div>
              <div style={{ flex: 1, display: 'flex', alignItems: 'center' }}>
                <CSticker s={s} dark={dark} size={64} rotation={rot * 1.5}/>
              </div>
              <div style={{ fontSize: 16, fontWeight: 800, textAlign: 'center', letterSpacing: '-.015em', textWrap: 'pretty' }}>{s.name}</div>
            </div>
          );
        })}
      </div>

      {/* Scrim only outside the highlighted tile */}
      <div style={{ position: 'absolute', inset: 0, background: dark ? 'rgba(0,0,0,.25)' : 'rgba(26,20,9,.18)', pointerEvents: 'none' }}/>

      {/* The context menu */}
      <div style={{
        position: 'absolute', left: anchorX, top: anchorY,
        width: 252, background: theme.panel, color: theme.ink,
        borderRadius: 14, padding: '6px 0',
        border: `1px solid ${theme.hairline2}`,
        boxShadow: '0 20px 50px rgba(0,0,0,.35), 0 4px 0 rgba(0,0,0,.05)',
        transform: 'rotate(-1deg)',
        overflow: 'hidden',
      }}>
        {/* Tiny pointer toward the tile */}
        <div style={{
          position: 'absolute', top: -7, left: 14, width: 14, height: 14,
          background: theme.panel, transform: 'rotate(45deg)',
          borderTop: `1px solid ${theme.hairline2}`, borderLeft: `1px solid ${theme.hairline2}`,
        }}/>

        {/* Sticker header */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '8px 14px 10px', borderBottom: `1px solid ${theme.hairline}` }}>
          <CSticker s={sound} dark={dark} size={32} rotation={-4}/>
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ fontSize: 13, fontWeight: 800, letterSpacing: '-.01em', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', fontStyle: 'italic' }}>{sound.name}</div>
            <div style={{ fontSize: 10.5, color: theme.inkDim, fontWeight: 600, textTransform: 'uppercase', letterSpacing: '.06em', marginTop: 1 }}>{sound.cat} · {sound.dur}</div>
          </div>
        </div>

        <div style={{ padding: '4px 0' }}>
          <Item icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5.5v13l11-6.5z"/></svg>} label="Play" shortcut="Space" hover/>
          <Item icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="3"/><path d="M12 5v2M12 17v2M5 12h2M17 12h2"/></svg>} label="Preview (you only)"/>
          <Sep/>
          <Item icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="#f59e0b" stroke="#f59e0b" strokeWidth="1"><path d="M12 2l3 7h7l-5.5 4.5L18 21l-6-4-6 4 1.5-7.5L2 9h7z"/></svg>} label="Remove from favorites"/>
          <Item icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M11 4l-7 7v6h6l7-7M14 7l3 3"/></svg>} label="Edit sound…" shortcut="↵"/>
          <Item icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><rect x="3" y="6" width="18" height="12" rx="2"/><path d="M7 10h.01M12 10h.01M17 10h.01"/></svg>} label="Bind to slot…" sub/>
          <Item icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><rect x="4" y="6" width="16" height="12" rx="2"/><path d="M8 10h.01M16 10h.01M8 14h8"/></svg>} label="Set hotkey…" shortcut={sound.hk || ''}/>
          <Sep/>
          <Item icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="3"/><path d="M12 1v6M12 17v6M4.2 4.2l4.3 4.3M15.5 15.5l4.3 4.3M1 12h6M17 12h6"/></svg>} label="Pick a sticker color…" sub/>
          <Item icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M12 5v14M5 12l7 7 7-7"/></svg>} label="Reveal in folder"/>
          <Item icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>} label="Copy file path"/>
          <Sep/>
          <Item icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M3 6h18M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/></svg>} label="Delete sound" shortcut="⌫" danger/>
        </div>
      </div>

      {/* tiny label at top */}
      <div style={{
        position: 'absolute', top: 18, left: 24, fontSize: 11, fontWeight: 700,
        color: theme.inkDim, letterSpacing: '.06em', textTransform: 'uppercase',
        background: theme.panel, padding: '4px 10px', borderRadius: 6,
        border: `1px solid ${theme.hairline2}`,
      }}>Right-click on a tile</div>
    </div>
  );
}

window.ContextMenu = ContextMenu;
