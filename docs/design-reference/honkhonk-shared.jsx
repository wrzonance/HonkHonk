// honkhonk-shared.jsx — shared data, icons, and small atoms used across all three directions.

// ─── Sound library data ────────────────────────────────────────────────────────
// Curated meme + utility sounds; durations and waveform seeds are deterministic
// so the mockup looks identical on every reload.
const HH_SOUNDS = [
  { id: 'vine-boom',      name: 'Vine Boom',          cat: 'Memes',     dur: '0:01', hk: 'F1',           tone: 'amber',  seed: 11 },
  { id: 'bruh',           name: 'Bruh',               cat: 'Memes',     dur: '0:01', hk: 'F2',           tone: 'orange', seed: 7  },
  { id: 'oof',            name: 'Roblox Oof',         cat: 'Memes',     dur: '0:01', hk: 'F3',           tone: 'amber',  seed: 4  },
  { id: 'sad-violin',     name: 'Sad Violin',         cat: 'Memes',     dur: '0:08', hk: null,           tone: 'rose',   seed: 22 },
  { id: 'metal-pipe',     name: 'Metal Pipe Falling', cat: 'Memes',     dur: '0:03', hk: 'F4',           tone: 'slate',  seed: 18 },
  { id: 'airhorn',        name: 'Airhorn',            cat: 'Memes',     dur: '0:02', hk: 'Ctrl+1',       tone: 'orange', seed: 3  },

  { id: 'discord-call',   name: 'Discord Incoming',   cat: 'Reactions', dur: '0:04', hk: null,           tone: 'violet', seed: 14 },
  { id: 'discord-leave',  name: 'Discord Leave',      cat: 'Reactions', dur: '0:01', hk: null,           tone: 'violet', seed: 9  },
  { id: 'wow',            name: 'Anime Wow',          cat: 'Reactions', dur: '0:01', hk: 'Ctrl+2',       tone: 'pink',   seed: 12 },
  { id: 'aughhh',         name: 'Aughhh',             cat: 'Reactions', dur: '0:02', hk: null,           tone: 'rose',   seed: 19 },

  { id: 'hello-there',    name: 'Hello There',        cat: 'Voicelines',dur: '0:02', hk: null,           tone: 'sky',    seed: 5  },
  { id: 'fus-ro-dah',     name: 'Fus Ro Dah',         cat: 'Voicelines',dur: '0:02', hk: 'Ctrl+3',       tone: 'sky',    seed: 21 },
  { id: 'wasted',         name: 'GTA Wasted',         cat: 'Voicelines',dur: '0:03', hk: null,           tone: 'red',    seed: 17 },
  { id: 'mission-pass',   name: 'Mission Passed',     cat: 'Voicelines',dur: '0:04', hk: null,           tone: 'green',  seed: 8  },

  { id: 'rickroll',       name: 'Rickroll Intro',     cat: 'Music',     dur: '0:18', hk: null,           tone: 'pink',   seed: 27 },
  { id: 'mii-channel',    name: 'Mii Channel',        cat: 'Music',     dur: '0:12', hk: null,           tone: 'sky',    seed: 31 },
  { id: 'all-star',       name: 'All Star',           cat: 'Music',     dur: '0:09', hk: null,           tone: 'green',  seed: 24 },
  { id: 'never-gonna',    name: 'Never Gonna…',       cat: 'Music',     dur: '0:11', hk: null,           tone: 'orange', seed: 28 },

  { id: 'goose-honk',     name: 'Goose Honk',         cat: 'Honk',      dur: '0:01', hk: 'F5',           tone: 'amber',  seed: 1  },
  { id: 'goose-honk-2',   name: 'Aggressive Honk',    cat: 'Honk',      dur: '0:02', hk: 'F6',           tone: 'amber',  seed: 2  },
  { id: 'goose-flock',    name: 'Goose Flock',        cat: 'Honk',      dur: '0:05', hk: null,           tone: 'amber',  seed: 6  },

  { id: 'bonk',           name: 'Bonk',               cat: 'SFX',       dur: '0:01', hk: null,           tone: 'slate',  seed: 13 },
  { id: 'pop',            name: 'Pop',                cat: 'SFX',       dur: '0:01', hk: null,           tone: 'sky',    seed: 15 },
  { id: 'whoosh',         name: 'Whoosh',             cat: 'SFX',       dur: '0:01', hk: null,           tone: 'sky',    seed: 25 },
];

const HH_CATEGORIES = ['All', 'Honk', 'Memes', 'Reactions', 'Voicelines', 'Music', 'SFX'];

// Tone tokens — each direction maps these onto its own palette. Keep names abstract.
const HH_TONES = {
  amber:  { hue: 38,  name: 'Amber'  },
  orange: { hue: 22,  name: 'Orange' },
  rose:   { hue: 350, name: 'Rose'   },
  pink:   { hue: 320, name: 'Pink'   },
  red:    { hue: 5,   name: 'Red'    },
  violet: { hue: 268, name: 'Violet' },
  sky:    { hue: 210, name: 'Sky'    },
  green:  { hue: 145, name: 'Green'  },
  slate:  { hue: 220, name: 'Slate'  },
};

// ─── Deterministic waveform generator ─────────────────────────────────────────
// Returns N normalized peak heights (0..1). Same seed = same waveform.
function hhWaveform(seed, n = 48) {
  const out = [];
  let s = (seed * 9301 + 49297) % 233280;
  for (let i = 0; i < n; i++) {
    s = (s * 9301 + 49297) % 233280;
    const noise = s / 233280;
    // soft envelope: louder in middle, taper at ends, with a few transient peaks
    const env = Math.sin((i / n) * Math.PI);
    const transient = (i % 7 === 0) ? 0.35 : 0;
    out.push(Math.max(0.08, Math.min(1, env * (0.35 + noise * 0.65) + transient)));
  }
  return out;
}

// ─── Goose mark (small svg) ───────────────────────────────────────────────────
function GooseMark({ size = 22, color = 'currentColor', accent = '#f59e0b', style }) {
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" style={style} aria-hidden>
      {/* body */}
      <path d="M9 22c0-4 3-7 7-7 1.5 0 3 .4 4 1l-1 4c-1-.5-2-.8-3-.8-2.5 0-4.5 1.8-4.5 4.3 0 .9.2 1.7.6 2.5H10c-.7 0-1-.5-1-1.2V22z" fill={color}/>
      {/* head + neck */}
      <path d="M19 6c2.5 0 4.5 2 4.5 4.5v3c0 1-.4 1.8-1.2 2.4l-3 2.1c-.5.4-1.2.5-1.8.3l-1.5-.5V12c0-.6.2-1.1.6-1.5l1.4-1.5V8.5C18 7.1 18.5 6 19 6z" fill={color}/>
      {/* beak */}
      <path d="M24 10.5l3 .8c.4.1.5.6.2.9l-2.8 1.5c-.3.2-.6 0-.6-.3v-2.5c0-.3.1-.4.2-.4z" fill={accent}/>
      {/* eye */}
      <circle cx="20.5" cy="9.5" r="0.8" fill="#1c1917"/>
    </svg>
  );
}

// ─── Tiny icons (stroke, currentColor) ────────────────────────────────────────
const HHIcon = {
  search: (p) => (<svg viewBox="0 0 24 24" width={p.s||16} height={p.s||16} fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/></svg>),
  play:   (p) => (<svg viewBox="0 0 24 24" width={p.s||16} height={p.s||16} fill="currentColor"><path d="M8 5.5v13l11-6.5z"/></svg>),
  stop:   (p) => (<svg viewBox="0 0 24 24" width={p.s||16} height={p.s||16} fill="currentColor"><rect x="6" y="6" width="12" height="12" rx="1.5"/></svg>),
  vol:    (p) => (<svg viewBox="0 0 24 24" width={p.s||16} height={p.s||16} fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M11 5 6 9H3v6h3l5 4z"/><path d="M16 8a5 5 0 0 1 0 8"/><path d="M19 5a9 9 0 0 1 0 14"/></svg>),
  volMute:(p) => (<svg viewBox="0 0 24 24" width={p.s||16} height={p.s||16} fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M11 5 6 9H3v6h3l5 4z"/><path d="m22 9-6 6M16 9l6 6"/></svg>),
  cog:    (p) => (<svg viewBox="0 0 24 24" width={p.s||16} height={p.s||16} fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3h.1a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8v.1a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/></svg>),
  grid:   (p) => (<svg viewBox="0 0 24 24" width={p.s||16} height={p.s||16} fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><rect x="3" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5"/><rect x="14" y="14" width="7" height="7" rx="1.5"/></svg>),
  list:   (p) => (<svg viewBox="0 0 24 24" width={p.s||16} height={p.s||16} fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M8 6h13M8 12h13M8 18h13M3.5 6h.01M3.5 12h.01M3.5 18h.01"/></svg>),
  mic:    (p) => (<svg viewBox="0 0 24 24" width={p.s||16} height={p.s||16} fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><rect x="9" y="3" width="6" height="12" rx="3"/><path d="M5 11a7 7 0 0 0 14 0M12 18v3"/></svg>),
  plus:   (p) => (<svg viewBox="0 0 24 24" width={p.s||16} height={p.s||16} fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M12 5v14M5 12h14"/></svg>),
  pause:  (p) => (<svg viewBox="0 0 24 24" width={p.s||16} height={p.s||16} fill="currentColor"><rect x="6" y="5" width="4" height="14" rx="1"/><rect x="14" y="5" width="4" height="14" rx="1"/></svg>),
};

// ─── Waveform mini-component ──────────────────────────────────────────────────
function HHWaveform({ seed, color, height = 28, progress = 0, playedColor }) {
  const bars = React.useMemo(() => hhWaveform(seed, 36), [seed]);
  return (
    <svg width="100%" height={height} viewBox={`0 0 ${bars.length * 3} ${height}`} preserveAspectRatio="none" style={{ display: 'block' }}>
      {bars.map((h, i) => {
        const bh = Math.max(2, h * (height - 4));
        const y = (height - bh) / 2;
        const isPlayed = i / bars.length < progress;
        return <rect key={i} x={i * 3} y={y} width={2} height={bh} rx={1} fill={isPlayed ? (playedColor || color) : color} />;
      })}
    </svg>
  );
}

Object.assign(window, { HH_SOUNDS, HH_CATEGORIES, HH_TONES, hhWaveform, GooseMark, HHIcon, HHWaveform });
