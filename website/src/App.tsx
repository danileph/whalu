import { useRef, useState } from "react";
import type {
  CSSProperties,
  KeyboardEvent,
  PointerEvent,
  SVGProps,
} from "react";

const repositoryUrl = (
  import.meta.env.VITE_REPOSITORY_URL as string | undefined
)?.replace(/\/$/, "");
const sourceUrl = repositoryUrl ? `${repositoryUrl}#readme` : "#get-whalu";

type IconName =
  | "arrow"
  | "github"
  | "apple"
  | "chevron"
  | "check"
  | "copy"
  | "plus"
  | "window"
  | "command";
function Icon({
  name,
  ...props
}: SVGProps<SVGSVGElement> & { name: IconName }) {
  const paths: Record<IconName, React.ReactNode> = {
    arrow: (
      <>
        <path d="M4 12h15M13 6l6 6-6 6" />
      </>
    ),
    github: (
      <path d="M9 19c-4.3 1.3-4.3-2-6-2.5M15 22v-3.4c0-1 .1-1.4-.5-2 3.3-.4 6.7-1.6 6.7-7.3a5.6 5.6 0 0 0-1.5-4 5.2 5.2 0 0 0-.1-4S18.4.9 15.4 3a14 14 0 0 0-7.6 0C4.8.9 3.6 1.3 3.6 1.3a5.2 5.2 0 0 0-.1 4 5.6 5.6 0 0 0-1.5 4c0 5.7 3.4 6.9 6.7 7.3-.5.5-.8 1.2-.7 2V22" />
    ),
    apple: (
      <path d="M15.2 4.1c.7-.9 1.2-2.1 1.1-3.3-1.1 0-2.4.7-3.2 1.5-.7.8-1.3 2-1.1 3.2 1.2.1 2.4-.6 3.2-1.4ZM19 12.4c0-2.8 2.3-4.1 2.4-4.2-1.3-2-3.4-2.2-4.1-2.3-1.8-.2-3.5 1.1-4.4 1.1-.9 0-2.3-1.1-3.8-1-2 0-3.8 1.2-4.8 2.9-2 3.5-.5 8.8 1.5 11.6 1 1.4 2.1 3 3.6 2.9 1.4-.1 2-1 3.8-1s2.3 1 3.8 1c1.6 0 2.6-1.4 3.5-2.8a12.5 12.5 0 0 0 1.6-3.3 4.8 4.8 0 0 1-3.1-4.9Z" />
    ),
    chevron: <path d="m9 5 7 7-7 7" />,
    check: <path d="m5 12 4 4L19 6" />,
    copy: (
      <>
        <rect x="8" y="8" width="12" height="13" rx="2" />
        <path d="M15 8V3H3v13h5" />
      </>
    ),
    plus: <path d="M12 4v16M4 12h16" />,
    window: (
      <>
        <rect x="3" y="4" width="18" height="16" rx="2" />
        <path d="M3 9h18M8 4v5" />
      </>
    ),
    command: (
      <path d="M8 8h8v8H8zm0 0H5a3 3 0 1 1 3-3zm8 0V5a3 3 0 1 1 3 3zm0 8h3a3 3 0 1 1-3 3zm-8 0v3a3 3 0 1 1-3-3z" />
    ),
  };
  return (
    <svg
      viewBox="0 0 24 24"
      fill={name === "apple" ? "currentColor" : "none"}
      stroke={name === "apple" ? "none" : "currentColor"}
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      {...props}
    >
      {paths[name]}
    </svg>
  );
}

function Brand({ large = false }: { large?: boolean }) {
  return (
    <a
      href="#"
      className={`brand${large ? " brand-large" : ""}`}
      aria-label="Whalu home"
    >
      <img
        src={`${import.meta.env.BASE_URL}whalu-icon.png`}
        width="40"
        height="40"
        alt=""
      />
      <span>
        whalu<span className="brand-dot">.</span>
      </span>
    </a>
  );
}

function Key({ children }: { children: React.ReactNode }) {
  return <kbd>{children}</kbd>;
}

function Landscape({ className = "" }: { className?: string }) {
  return (
    <svg
      className={`landscape ${className}`}
      viewBox="0 0 1000 700"
      preserveAspectRatio="xMidYMid slice"
      aria-hidden="true"
    >
      <rect width="1000" height="700" fill="#c7c8dd" />
      <circle cx="742" cy="150" r="72" fill="#ece4d8" />
      <path
        d="M-100 560C90 530 176 292 360 310S630 540 1100 158V760H-100Z"
        fill="#8b949b"
      />
      <path
        d="M-100 599C125 520 297 594 487 427S751 262 1100 408V760H-100Z"
        fill="#697d76"
      />
      <path
        d="M-100 710C174 575 341 700 548 555S845 393 1100 464V760H-100Z"
        fill="#3f5b50"
      />
      <path
        d="M250 760C426 648 516 676 685 584S903 515 1100 563"
        fill="none"
        stroke="#adc0a1"
        strokeWidth="1"
        opacity=".5"
      />
      <path
        d="M330 780C486 678 581 705 750 613S961 547 1140 585"
        fill="none"
        stroke="#adc0a1"
        strokeWidth="1"
        opacity=".35"
      />
    </svg>
  );
}

const windows = [
  {
    id: "finder",
    app: "Finder",
    title: "A few good projects",
    color: "#6d9bc2",
    mark: "F",
  },
  {
    id: "code",
    app: "Editor",
    title: "Something in the making",
    color: "#756da7",
    mark: "⌘",
  },
  {
    id: "browser",
    app: "Safari",
    title: "The quiet desk",
    color: "#528cbd",
    mark: "↗",
  },
  {
    id: "notes",
    app: "Notes",
    title: "A little room to think",
    color: "#bc9b4d",
    mark: "≡",
  },
  {
    id: "terminal",
    app: "Terminal",
    title: "All systems go",
    color: "#4b6156",
    mark: ">_",
  },
] as const;
type WindowId = (typeof windows)[number]["id"];

function WindowContent({
  id,
  small = false,
}: {
  id: WindowId;
  small?: boolean;
}) {
  if (id === "browser")
    return (
      <div className={`browser-content${small ? " mini" : ""}`}>
        <div className="journal-nav">
          <span>OFFSCREEN JOURNAL</span>
          <span>
            Ideas for a slower day <span className="journal-dot">●</span>
          </span>
        </div>
        <div className="journal-body">
          <div className="journal-copy">
            <span className="tiny-label">ISSUE 008 — AT HOME</span>
            <h3>
              The quiet
              <br />
              <em>desk.</em>
            </h3>
            <p>
              A little space to make
              <br />
              something good.
            </p>
            <span className="journal-link">
              Take a moment <span>↗</span>
            </span>
          </div>
          <div className="journal-art">
            <Landscape />
            <span>A DIFFERENT KIND OF VIEW</span>
          </div>
        </div>
        <div className="journal-bottom">
          <span>LESS, BUT A LITTLE BETTER.</span>
          <span>08 / 12</span>
        </div>
      </div>
    );
  if (id === "code")
    return (
      <div className="code-content">
        <aside>
          <span>EXPLORER</span>
          <p>⌄ whalu</p>
          <p className="code-indent">⌄ src</p>
          <p className="code-file">◇ main.rs</p>
          <p className="code-indent">◇ navigation.rs</p>
          <p>◇ Cargo.toml</p>
        </aside>
        <div className="code-editor">
          <div className="code-tab">
            ◇ main.rs <span>×</span>
          </div>
          <pre>
            <span className="code-comment">
              // A small utility. A little more focus.
            </span>
            {"\n\n"}
            <span className="code-purple">fn</span>{" "}
            <span className="code-yellow">main</span>() {"{"}
            {"\n"} <span className="code-purple">let</span> desktop = Desktop::
            <span className="code-yellow">current</span>();{"\n\n"}{" "}
            <span className="code-purple">for</span> window{" "}
            <span className="code-purple">in</span> desktop.
            <span className="code-yellow">windows</span>() {"{"}
            {"\n"} window.<span className="code-yellow">find_your_place</span>
            ();{"\n"} {"}"}
            {"\n"}
            {"}"}
            <span className="code-cursor" />
          </pre>
          <span className="code-status">
            main* <span>Rust · UTF-8</span>
          </span>
        </div>
      </div>
    );
  if (id === "notes")
    return (
      <div className="notes-content">
        <span className="note-date">FRIDAY, 10:24 AM</span>
        <h3>
          A little room
          <br />
          to <em>think.</em>
        </h3>
        <p>Things worth making space for:</p>
        <ul>
          <li>
            <span>✓</span> A fresh perspective
          </li>
          <li>
            <span>✓</span> One thing at a time
          </li>
          <li>
            <span className="empty-check" /> The next good idea
          </li>
        </ul>
        <div className="note-sketch">
          <svg viewBox="0 0 240 80" aria-hidden="true">
            <path
              d="M10 54C62-8 145 3 162 39c20 43-74 42-68 6 7-39 104-34 132-5M202 20l25 20-26 11"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
            />
          </svg>
          <span>start somewhere.</span>
        </div>
      </div>
    );
  if (id === "finder")
    return (
      <div className="finder-content">
        <aside>
          <span>Favorites</span>
          <p>◷ Recents</p>
          <p>☁ iCloud Drive</p>
          <p className="finder-current">▱ Projects</p>
          <p>↓ Downloads</p>
          <p>⌂ Home</p>
        </aside>
        <div className="finder-files">
          {[
            "Whalu",
            "Weekend ideas",
            "Field notes",
            "Good things",
            "Experiments",
            "Someday",
          ].map((name) => (
            <div key={name}>
              <span className="folder" />
              <span>{name}</span>
            </div>
          ))}
        </div>
      </div>
    );
  return (
    <div className="terminal-content">
      <p>
        <span>~/projects/whalu</span> on main
      </p>
      <p>❯ cargo build --release</p>
      <p className="terminal-muted"> Compiling whalu v0.5.1</p>
      <p>
        <span> Finished</span> release profile
      </p>
      <p>
        ❯ <span className="terminal-cursor" />
      </p>
      <div className="terminal-aside">
        Less hunting.
        <br />
        More doing.
      </div>
    </div>
  );
}

function TrafficLights() {
  return (
    <span className="traffic-lights" aria-hidden="true">
      <i />
      <i />
      <i />
    </span>
  );
}

function DesktopDemo() {
  const [selected, setSelected] = useState(2);
  const [interacted, setInteracted] = useState(false);
  const touchStart = useRef<{ x: number; y: number } | null>(null);
  const wheelTime = useRef(0);
  const selectedWindow = windows[selected];

  function select(index: number) {
    setSelected(Math.max(0, Math.min(windows.length - 1, index)));
    setInteracted(true);
  }
  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const target = event.target as HTMLElement;
    if (target !== event.currentTarget && !target.closest('.wheel-card')) return;
    if (
      [
        "ArrowDown",
        "ArrowRight",
        "j",
        "ArrowUp",
        "ArrowLeft",
        "k",
        "Home",
        "End",
      ].includes(event.key)
    ) {
      event.preventDefault();
      // Keep keyboard focus visible as cards move beyond the clipped wheel.
      event.currentTarget.focus({ preventScroll: true });
      const next = ["ArrowDown", "ArrowRight", "j"].includes(event.key);
      const index =
        event.key === "Home"
          ? 0
          : event.key === "End"
            ? windows.length - 1
            : (selected + (next ? 1 : -1) + windows.length) % windows.length;
      select(index);
    }
  }
  function pointerUp(event: PointerEvent<HTMLDivElement>) {
    if (!touchStart.current) return;
    const dx = event.clientX - touchStart.current.x;
    const dy = event.clientY - touchStart.current.y;
    if (Math.max(Math.abs(dx), Math.abs(dy)) > 38)
      select(
        selected + ((Math.abs(dx) > Math.abs(dy) ? -dx : -dy) > 0 ? 1 : -1),
      );
    touchStart.current = null;
  }

  return (
    <div className="demo-wrap" id="try-it">
      <div
        className="desktop"
        tabIndex={0}
        role="region"
        aria-label="Interactive Whalu demo. Use arrow keys, select a thumbnail, or swipe to switch windows."
        aria-describedby="demo-instructions"
        onKeyDown={onKeyDown}
        onPointerDown={(event) => {
          if (event.pointerType === "touch")
            touchStart.current = { x: event.clientX, y: event.clientY };
        }}
        onPointerUp={pointerUp}
        onPointerCancel={() => {
          touchStart.current = null;
        }}
      >
        <Landscape className="wallpaper" />
        <div className="desktop-grain" />
        <div className="mac-menu" aria-hidden="true">
          <div>
            <Icon name="apple" />
            <strong>{selectedWindow.app}</strong>
            <span>File</span>
            <span>Edit</span>
            <span>View</span>
            <span>Window</span>
          </div>
          <div>
            <span className="menubar-whalu">W</span>
            <span>◒</span>
            <span>Fri 10:24</span>
          </div>
        </div>
        <div className="desktop-space" aria-hidden="true">
          <span className="space-dot" /> DESKTOP 01
        </div>
        <div className="background-window" aria-hidden="true">
          <div className="window-toolbar">
            <TrafficLights />
            <span>Good things take a little space.</span>
          </div>
          <WindowContent id="notes" small />
        </div>
        <div
          className="active-window"
          key={selectedWindow.id}
          aria-hidden="true"
        >
          <div className="window-toolbar">
            <TrafficLights />
            {selectedWindow.id === "browser" ? (
              <div className="browser-address">
                <span>◈</span> offscreen.journal <span>↻</span>
              </div>
            ) : (
              <span>{selectedWindow.title}</span>
            )}
            <span className="toolbar-detail">⊞</span>
          </div>
          <WindowContent id={selectedWindow.id} />
        </div>
        <div
          className="window-wheel"
          aria-label="Window thumbnails"
          onWheel={(event) => {
            if (
              Math.abs(event.deltaY) < 5 ||
              Date.now() - wheelTime.current < 220
            )
              return;
            wheelTime.current = Date.now();
            select(selected + Math.sign(event.deltaY));
          }}
        >
          {windows.map((window, index) => {
            const distance = index - selected;
            const depth = Math.abs(distance);
            const style = {
              "--offset": distance,
              "--depth": depth,
              "--y": `${distance * (depth > 1 ? 104 : 122)}px`,
              "--scale": 1 - Math.min(depth, 4) * 0.15,
              "--tilt": `${distance * -8}deg`,
              zIndex: 10 - depth,
            } as CSSProperties;
            return (
              <button
                className={`wheel-card${distance === 0 ? " is-selected" : ""}`}
                key={window.id}
                style={style}
                onClick={() => select(index)}
                aria-label={`Switch to ${window.app}: ${window.title}`}
                aria-pressed={distance === 0}
              tabIndex={distance === 0 ? 0 : -1}
              >
                <span className="wheel-card-preview" aria-hidden="true">
                  <span className="mini-toolbar">
                    <TrafficLights />
                    <span>{window.app}</span>
                  </span>
                  <WindowContent id={window.id} small />
                </span>
                <span
                  className="app-badge"
                  aria-hidden="true"
                  style={{ "--app-color": window.color } as CSSProperties}
                >
                  {window.mark}
                </span>
              </button>
            );
          })}
        </div>
        <div className="demo-hint" aria-hidden="true">
          <span className="hint-pointer">↖</span>
          {interacted ? "There it is. Keep going." : "Go on. Pick a window."}
        </div>
        <div className="desktop-dock" aria-hidden="true">
          {windows.map((window, index) => (
            <span
              className={`dock-icon${selected === index ? " active" : ""}`}
              key={window.id}
              style={{ backgroundColor: window.color }}
            >
              {window.mark}
            </span>
          ))}
          <span className="dock-divider" />
          <img src={`${import.meta.env.BASE_URL}whalu-icon.png`} alt="" />
        </div>
      </div>
      <div className="demo-caption">
        <div>
          <span className="live-dot" />
          <span id="demo-instructions">
            A little demo.{" "}
            <span className="caption-muted">
              Click a window. Find your way.
            </span>
          </span>
        </div>
        <div className="demo-pagination">
          <span className="demo-counter" aria-live="polite" aria-atomic="true">
            0{selected + 1} <span>/ 05</span>
            <span className="sr-only"> — {selectedWindow.app}</span>
          </span>
          <button
            onClick={() => select(selected - 1)}
            disabled={selected === 0}
            aria-label="Previous window"
          >
            <Icon name="arrow" className="arrow-back" />
          </button>
          <button
            onClick={() => select(selected + 1)}
            disabled={selected === windows.length - 1}
            aria-label="Next window"
          >
            <Icon name="arrow" />
          </button>
        </div>
      </div>
    </div>
  );
}

const controls = [
  {
    group: "Move through your windows",
    rows: [
      {
        label: "Previous / next window",
        keys: ["⌃", "⌥", "↑ / ↓"],
        alternate: "or K / J",
      },
      {
        label: "Same app, different window",
        keys: ["⌃", "⌥", "← / →"],
        alternate: "or H / L",
      },
      {
        label: "The familiar switcher, window by window",
        keys: ["⌘", "Tab"],
        alternate: "",
      },
    ],
  },
  {
    group: "Put things in their place",
    rows: [
      {
        label: "Move a window in the stack",
        keys: ["⌃", "⌥", "⌘", "↑ / ↓"],
        alternate: "",
      },
      {
        label: "Send a window to an adjacent Space",
        keys: ["⌃", "⌥", "⌘", "← / →"],
        alternate: "",
      },
      {
        label: "Tile to the left / right half",
        keys: ["⌃", "⌘", "← / →"],
        alternate: "",
      },
    ],
  },
];

function Controls() {
  const [expanded, setExpanded] = useState(false);
  return (
    <section
      className="controls-section"
      id="controls"
      aria-labelledby="controls-heading"
    >
      <div className="section-heading">
        <span className="eyebrow">
          <span className="section-number">02 /</span> SECOND NATURE
        </span>
        <div>
          <h2 id="controls-heading">
            Your hands already
            <br />
            <em>know the way.</em>
          </h2>
          <p>Keyboard person? Trackpad person? Whalu meets you there.</p>
        </div>
      </div>
      <div className="input-methods">
        <div className="input-method">
          <div
            className="input-illustration keyboard-illustration"
            aria-hidden="true"
          >
            <div className="keyboard-row">
              <Key>⌃</Key>
              <Key>⌥</Key>
              <Key>↓</Key>
            </div>
            <span className="input-line" />
          </div>
          <span className="tiny-label">01 — THE KEYS</span>
          <h3>Keep your rhythm.</h3>
          <p>
            Hold Control + Option for the overview. Arrow through individual
            windows, or use H, J, K, L. Even Command + Tab joins in.
          </p>
        </div>
        <div className="input-method">
          <div
            className="input-illustration trackpad-illustration"
            aria-hidden="true"
          >
            <div className="trackpad">
              <i />
              <i />
              <i />
              <span>↑</span>
            </div>
          </div>
          <span className="tiny-label">02 — THE TRACKPAD</span>
          <h3>A familiar feeling.</h3>
          <p>
            Rest three fingers to take a look. Swipe to move between windows.
            Lift your fingers and get right back to it.
          </p>
        </div>
        <div className="input-method">
          <div
            className="input-illustration mouse-illustration"
            aria-hidden="true"
          >
            <div className="mouse">
              <span />
              <i />
            </div>
            <div className="mouse-scroll">↕</div>
          </div>
          <span className="tiny-label">03 — THE MOUSE</span>
          <h3>Just passing through.</h3>
          <p>
            Visit the left edge to reveal your windows. Scroll through the
            overview, or click exactly where you want to go.
          </p>
        </div>
      </div>
      <button
        className="shortcut-toggle"
        onClick={() => setExpanded(!expanded)}
        aria-expanded={expanded}
        aria-controls="shortcut-sheet"
      >
        <span>
          {expanded
            ? "Put the cheat sheet away"
            : "A small shortcut cheat sheet"}
        </span>
        <Icon name="plus" className={expanded ? "rotate-45" : ""} />
      </button>
      <div id="shortcut-sheet" hidden={!expanded} className="shortcut-sheet">
        {controls.map((group) => (
          <div key={group.group}>
            <h3>{group.group}</h3>
            {group.rows.map((row) => (
              <div className="shortcut-row" key={row.label}>
                <span>{row.label}</span>
                <span className="shortcut-keys">
                  {row.keys.map((key) => (
                    <Key key={key}>{key}</Key>
                  ))}
                  {row.alternate && <small>{row.alternate}</small>}
                </span>
              </div>
            ))}
          </div>
        ))}
        <p>
          All bindings are configurable. Keep native Space and Mission Control
          gestures on four fingers, with three-finger dragging disabled.
        </p>
      </div>
    </section>
  );
}

function NativeArrangement() {
  const [side, setSide] = useState<"left" | "right">("left");
  return (
    <div className={`tiling-demo tile-${side}`}>
      <div className="tiling-screen" aria-hidden="true">
        <span className="tiling-mini-menu" />
        <div className="tiling-window tiling-back">
          <span />
          <i />
          <i />
          <i />
        </div>
        <div className="tiling-window tiling-front">
          <div>
            <i />
            <i />
            <i />
          </div>
          <span />
          <span />
          <span />
          <span />
        </div>
      </div>
      <div className="tiling-controls">
        <span>Let macOS do the fitting.</span>
        <div>
          <button
            aria-label="Tile window to the left"
            aria-pressed={side === "left"}
            onClick={() => setSide("left")}
          >
            ←
          </button>
          <button
            aria-label="Tile window to the right"
            aria-pressed={side === "right"}
            onClick={() => setSide("right")}
          >
            →
          </button>
        </div>
      </div>
    </div>
  );
}

function Install() {
  const [copied, setCopied] = useState(false);
  const [copyError, setCopyError] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  async function copyCommand() {
    try {
      await navigator.clipboard.writeText("./scripts/install.sh");
      setCopied(true);
      setCopyError(false);
      clearTimeout(timer.current);
      timer.current = setTimeout(() => setCopied(false), 2500);
    } catch {
      setCopyError(true);
    }
  }
  return (
    <section
      className="install-section"
      id="get-whalu"
      aria-labelledby="install-heading"
    >
      <div className="install-topline">
        <span className="eyebrow">
          <span className="section-number">04 /</span> MAKE YOURSELF AT HOME
        </span>
        <span className="version-tag">v0.5.1 · SOURCE BUILD</span>
      </div>
      <div className="install-grid">
        <div className="install-copy">
          <img
            src={`${import.meta.env.BASE_URL}whalu-icon.png`}
            width="80"
            height="80"
            alt="Whalu's silver window panes and orbit app icon"
          />
          <h2 id="install-heading">
            A little utility.
            <br />
            <em>A lighter desktop.</em>
          </h2>
          <p>
            Free to use. Open to tinker with.
            <br />
            Make a little space for Whalu.
          </p>
          {repositoryUrl && (
            <a
              className="text-link"
              href={sourceUrl}
              target="_blank"
              rel="noreferrer"
            >
              Explore the source <Icon name="arrow" />
            </a>
          )}
        </div>
        <div className="install-steps">
          <div className="install-step">
            <span className="step-number">1</span>
            <div>
              <h3>Build your own.</h3>
              <p>
                {repositoryUrl ? (
                  <>
                    Clone{" "}
                    <a href={repositoryUrl} target="_blank" rel="noreferrer">
                      the repository
                    </a>
                    , then run this from its folder.
                  </>
                ) : (
                  <>From your local Whalu source folder, run:</>
                )}
              </p>
              <div className="terminal-command">
                <span aria-hidden="true">$</span>
                <code>./scripts/install.sh</code>
                <button
                  onClick={copyCommand}
                  aria-label={
                    copied ? "Command copied" : "Copy install command"
                  }
                >
                  <Icon name={copied ? "check" : "copy"} />
                </button>
              </div>
              <span className="copy-status" role="status">
                {copied
                  ? "Copied. Your terminal awaits."
                  : copyError
                    ? "Select and copy the command above."
                    : "Requires Rust + Apple’s Command Line Tools."}
              </span>
            </div>
          </div>
          <div className="install-step">
            <span className="step-number">2</span>
            <div>
              <h3>Give it a warm welcome.</h3>
              <p>
                Enable Whalu in Accessibility. Allow Screen &amp; System Audio
                Recording for window previews. Whalu will guide you through it.
              </p>
            </div>
          </div>
          <div className="install-step">
            <span className="step-number">3</span>
            <div>
              <h3>Find your way.</h3>
              <p>
                Hold <Key>⌃</Key> + <Key>⌥</Key>. There are your windows.
                <br />
                We’ll leave you to it.
              </p>
            </div>
          </div>
        </div>
      </div>
      <div className="install-footnote">
        <span>
          <Icon name="apple" /> macOS 15.0 or later
        </span>
        <span>Built with Rust. Right at home on your Mac.</span>
      </div>
    </section>
  );
}

function Faq() {
  const faqs = [
    [
      "Will it rearrange my desktop?",
      "Your windows stay where you put them. Whalu changes focus and the order of its window stack. Tiling happens only when you ask for it, using macOS’s own window commands.",
    ],
    [
      "What happens to my window previews?",
      "Whalu takes small snapshots of individual windows while the overview is open. They stay in memory on your Mac. It doesn’t record audio, save captures to disk, or upload them. Without permission, you’ll see app icons instead.",
    ],
    [
      "Does it work with multiple displays and Spaces?",
      "Each display keeps its own window order and remembered selection across Spaces. The overview opens on the display under your pointer and shows windows in its current Space. You can also send the focused window to an adjacent desktop with a shortcut.",
    ],
    [
      "Can I make it my own?",
      "Yes. Open the configuration from the W menu to customize shortcuts, turn gestures on or off, or ignore specific apps. Reload the configuration from that same menu. Whalu is open source under the MIT license.",
    ],
  ];
  return (
    <section className="faq-section" aria-labelledby="faq-heading">
      <div>
        <span className="eyebrow">A FEW THINGS, BEFORE YOU GO</span>
        <h2 id="faq-heading">
          Good to <em>know.</em>
        </h2>
      </div>
      <div className="faq-list">
        {faqs.map(([question, answer]) => (
          <details key={question}>
            <summary>
              {question}
              <Icon name="plus" />
            </summary>
            <p>{answer}</p>
          </details>
        ))}
      </div>
    </section>
  );
}

export function App() {
  return (
    <>
      <a className="skip-link" href="#main">
        Skip to content
      </a>
      <div className="site-shell">
        <header className="site-header">
          <Brand />
          <nav aria-label="Main navigation">
            <a href="#a-better-overview">The idea</a>
            <a href="#controls">The controls</a>
            <a href="#get-whalu" className="nav-cta">
              Get Whalu <Icon name="arrow" />
            </a>
          </nav>
        </header>
        <main id="main">
          <section className="hero" aria-labelledby="hero-heading">
            <div className="hero-topline">
              <span className="eyebrow">
                <span className="status-dot" /> A QUIET LITTLE WINDOW UTILITY
              </span>
              <span className="hero-platform">
                MADE FOR YOUR MAC <span>↙</span>
              </span>
            </div>
            <div className="hero-intro">
              <h1 id="hero-heading">
                Less hunting.
                <br />
                More <em>doing.</em>
              </h1>
              <div className="hero-copy">
                <p>
                  Your next window is closer than you think. Meet a calmer way
                  to move around your Mac.
                </p>
                <a href="#get-whalu" className="button button-dark">
                  <Icon name="apple" /> Make room for Whalu{" "}
                  <Icon name="arrow" />
                </a>
                <span className="hero-note">
                  Free &amp; open source <span>·</span> macOS 15+
                </span>
              </div>
            </div>
            <DesktopDemo />
            <div className="hero-bottom">
              <span>INDIVIDUAL WINDOWS. A LITTLE PERSPECTIVE.</span>
              <a href="#a-better-overview">
                A closer look <span>↓</span>
              </a>
            </div>
          </section>
          <section
            className="overview-section"
            id="a-better-overview"
            aria-labelledby="overview-heading"
          >
            <span className="eyebrow">
              <span className="section-number">01 /</span> SEE THE WHOLE PICTURE
            </span>
            <div className="overview-intro">
              <h2 id="overview-heading">
                Ten windows open.
                <br />
                <em>One train of thought.</em>
              </h2>
              <div>
                <p>
                  You know it’s in here somewhere. That document. That other
                  browser window. The thing you were just working on.
                </p>
                <p>
                  Whalu brings individual windows into a single, scrollable
                  overview. Take a look, pick your place, and carry on.
                </p>
              </div>
            </div>
            <div className="overview-notes">
              <div>
                <span>01</span>
                <h3>Windows, individually.</h3>
                <p>
                  Three windows in one app? They each get a place. Get straight
                  to the one you need.
                </p>
              </div>
              <div>
                <span>02</span>
                <h3>Your order of things.</h3>
                <p>
                  Drag previews to reorder your stack. Your focus and desktop
                  layout stay just as they were.
                </p>
              </div>
              <div>
                <span>03</span>
                <h3>Only here when you need it.</h3>
                <p>
                  A shortcut, a swipe, a visit to the edge. The overview
                  appears, then gets out of your way.
                </p>
              </div>
            </div>
          </section>
          <Controls />
          <section className="native-section" aria-labelledby="native-heading">
            <div className="native-copy">
              <span className="eyebrow">
                <span className="section-number">03 /</span> A GOOD MAC CITIZEN
              </span>
              <h2 id="native-heading">
                Still your Mac.
                <br />
                <em>Just a bit tidier.</em>
              </h2>
              <p>
                Put a window to the left. Give another the whole screen. Whalu
                calls the native macOS tiling commands, so things fit the way
                your Mac intended.
              </p>
              <a className="text-link" href="#get-whalu">
                Make yourself at home <Icon name="arrow" />
              </a>
            </div>
            <NativeArrangement />
          </section>
          <section className="quiet-section" aria-label="Whalu's approach">
            <span className="asterisk" aria-hidden="true">
              ✳
            </span>
            <p>
              Small by design.
              <br />
              <span>No account. No cloud. Just your windows.</span>
            </p>
            <span className="quiet-note">
              PREVIEWS STAY ON YOUR MAC.
              <br />
              GOOD THINGS CAN BE SIMPLE.
            </span>
          </section>
          <Install />
          <Faq />
        </main>
        <footer className="site-footer">
          <div>
            <Brand />
            <span>A little less between you and your work.</span>
          </div>
          <div>
            <a
              href={sourceUrl}
              {...(repositoryUrl
                ? { target: "_blank", rel: "noreferrer" }
                : {})}
            >
              {repositoryUrl ? (
                <>
                  <Icon name="github" /> GitHub
                </>
              ) : (
                "Build from source ↗"
              )}
            </a>
            <span>Open source, MIT licensed.</span>
            <a href="#" className="back-top" aria-label="Back to top">
              ↑
            </a>
          </div>
        </footer>
      </div>
    </>
  );
}
