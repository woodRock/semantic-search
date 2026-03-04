import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow, LogicalSize } from "@tauri-apps/api/webviewWindow";
import "./App.css";

interface SearchResult {
  path: string;
  score: number;
  snippet: string;
}

interface ProgressEvent {
  message: string;
  current: number;
  total: number;
}

interface Settings {
  ignored_paths: string[];
  ollama_url: string;
  theme: string;
}

function App() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [indexing, setIndexing] = useState(false);
  const [searching, setSearching] = useState(false);
  const [message, setMessage] = useState("");
  const [progress, setProgress] = useState(0);
  
  // Spotlight/Keyboard navigation
  const [activeIndex, setActiveIndex] = useState(-1);
  const resultListRef = useRef<HTMLUListElement>(null);

  // Chat state
  const [chatQuery, setChatQuery] = useState("");
  const [chatResponse, setChatResponse] = useState("");
  const [isChatting, setIsChatting] = useState(false);

  // Settings state
  const [showSettings, setShowSettings] = useState(false);
  const [settings, setSettings] = useState<Settings>({ 
    ignored_paths: [], 
    ollama_url: "http://localhost:11434",
    theme: "system"
  });
  const [newIgnorePath, setNewIgnorePath] = useState("");

  const [fileFilter, setFileFilter] = useState("");
  const [isRegex, setIsRegex] = useState(false);
  const [showFilters, setShowFilters] = useState(false);

  // Dynamic window resizing
  const isExpanded = results.length > 0 || searching || !!message || showSettings;

  useEffect(() => {
    const appWindow = getCurrentWebviewWindow();
    if (isExpanded) {
      appWindow.setSize(new LogicalSize(750, 550));
    } else {
      appWindow.setSize(new LogicalSize(750, 100));
    }
  }, [isExpanded]);

  useEffect(() => {
    fetchSettings();

    const unlisten = listen<ProgressEvent>("indexing-progress", (event) => {
      setMessage(event.payload.message);
      if (event.payload.total > 0) {
        setProgress((event.payload.current / event.payload.total) * 100);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    if (settings.theme === "dark") {
      document.body.classList.add("dark-theme");
      document.body.classList.remove("light-theme");
    } else if (settings.theme === "light") {
      document.body.classList.add("light-theme");
      document.body.classList.remove("dark-theme");
    } else {
      document.body.classList.remove("dark-theme", "light-theme");
    }
  }, [settings.theme]);

  useEffect(() => {
    if (activeIndex >= 0 && resultListRef.current) {
      const activeEl = resultListRef.current.children[activeIndex] as HTMLElement;
      if (activeEl) {
        activeEl.scrollIntoView({ block: "nearest" });
      }
    }
  }, [activeIndex]);

  async function fetchSettings() {
    try {
      const s = await invoke<Settings>("get_settings");
      setSettings(s);
    } catch (e) {
      console.error("Failed to fetch settings:", e);
    }
  }

  async function saveSettings(newSettings: Settings) {
    try {
      await invoke("update_settings", { settings: newSettings });
      setSettings(newSettings);
    } catch (e) {
      console.error("Failed to save settings:", e);
      setMessage(`Failed to save settings: ${e}`);
    }
  }

  function handleAddIgnore() {
    if (!newIgnorePath) return;
    const newSettings = {
      ...settings,
      ignored_paths: [...settings.ignored_paths, newIgnorePath]
    };
    saveSettings(newSettings);
    setNewIgnorePath("");
  }

  function handleRemoveIgnore(path: string) {
    const newSettings = {
      ...settings,
      ignored_paths: settings.ignored_paths.filter(p => p !== path)
    };
    saveSettings(newSettings);
  }

  async function handlePickDirectory() {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Select Directory to Index",
      });
      
      if (selected && typeof selected === "string") {
        setIndexing(true);
        setMessage(`Preparing to index ${selected}...`);
        setProgress(0);
        await invoke("index_directory", { dirPath: selected });
        setIndexing(false);
      }
    } catch (e) {
      console.error(e);
      setMessage(`Error: ${e}`);
      setIndexing(false);
    }
  }

  async function handleSearch() {
    if (!query) return;
    setSearching(true);
    setActiveIndex(-1);
    setChatResponse(""); // Reset chat when searching again
    try {
      const searchResults = await invoke<SearchResult[]>("search", { 
        query,
        fileTypeFilter: fileFilter ? fileFilter : null,
        isRegex
      });
      setResults(searchResults);
    } catch (e) {
      console.error(e);
      setMessage(`Error: ${e}`);
    } finally {
      setSearching(false);
    }
  }

  async function handleChat() {
    if (!chatQuery || results.length === 0) return;
    setIsChatting(true);
    setChatResponse("Thinking...");
    try {
      // Send the top 5 snippets as context
      const context = results.slice(0, 5).map(r => r.snippet);
      const res = await invoke<string>("ask_question", { query: chatQuery, context });
      setChatResponse(res);
    } catch (e) {
      console.error(e);
      setChatResponse(`Error connecting to Ollama: ${e}`);
    } finally {
      setIsChatting(false);
    }
  }

  async function openPath(path: string) {
    try {
      await invoke("open_path", { path });
    } catch (e) {
      console.error("Failed to open file:", e);
      setMessage(`Failed to open file: ${e}`);
    }
  }

  function handleKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Enter") {
      if (activeIndex >= 0 && results[activeIndex]) {
        openPath(results[activeIndex].path);
      } else {
        handleSearch();
      }
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      setActiveIndex(prev => Math.min(prev + 1, results.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActiveIndex(prev => Math.max(prev - 1, -1));
    } else if (e.key === "Escape") {
      setQuery("");
      setResults([]);
      setActiveIndex(-1);
      setChatResponse("");
      setShowSettings(false);
    }
  }

  return (
    <main className="container spotlight-container">
      {isExpanded && (
        <div className="header">
          <h1>Semantic Search</h1>
          <button className="settings-toggle" onClick={() => setShowSettings(!showSettings)}>
            {showSettings ? "✕ Close Settings" : "⚙ Settings"}
          </button>
        </div>
      )}
      
      {showSettings ? (
        <div className="settings-panel">
          <h2>Settings</h2>
          
          <div className="setting-group">
            <label>Ollama Base URL</label>
            <input 
              value={settings.ollama_url}
              onChange={(e) => saveSettings({ ...settings, ollama_url: e.target.value })}
              placeholder="http://localhost:11434"
            />
            <small>If Ollama is offline, search falls back to keyword-only mode.</small>
          </div>

          <div className="setting-group">
            <label>Theme</label>
            <select 
              value={settings.theme} 
              onChange={(e) => saveSettings({ ...settings, theme: e.target.value })}
              className="theme-select"
            >
              <option value="system">System Default</option>
              <option value="light">Light</option>
              <option value="dark">Dark</option>
            </select>
          </div>

          <div className="setting-group">
            <label>Ignore List</label>
            <p className="description">Files or directories matching these patterns will be skipped (e.g., "node_modules").</p>
            <div className="settings-input-group">
              <input 
                value={newIgnorePath}
                onChange={(e) => setNewIgnorePath(e.target.value)}
                placeholder="Add pattern to ignore..."
                onKeyDown={(e) => e.key === "Enter" && handleAddIgnore()}
              />
              <button onClick={handleAddIgnore}>Add</button>
            </div>
            <ul className="ignore-list">
              {settings.ignored_paths.map((path, i) => (
                <li key={i}>
                  <span>{path}</span>
                  <button className="remove-btn" onClick={() => handleRemoveIgnore(path)}>Remove</button>
                </li>
              ))}
              {settings.ignored_paths.length === 0 && <li>No ignored paths configured.</li>}
            </ul>
          </div>
        </div>
      ) : (
        <>
          <div className="search-bar spotlight-search">
            <input
              autoFocus
              value={query}
              onChange={(e) => {
                setQuery(e.target.value);
                setActiveIndex(-1);
              }}
              placeholder="Search your files... (Hit Enter)"
              onKeyDown={handleKeyDown}
            />
            <button className="filter-toggle" onClick={() => setShowFilters(!showFilters)}>
              {showFilters ? "▲" : "▼"}
            </button>
            {searching && <div className="spinner"></div>}
          </div>

          {showFilters && (
            <div className="advanced-filters">
              <label>
                <input 
                  type="checkbox" 
                  checked={isRegex} 
                  onChange={(e) => setIsRegex(e.target.checked)} 
                />
                Regex Mode
              </label>
              <input 
                className="ext-filter"
                placeholder="Extension (e.g. .rs)"
                value={fileFilter}
                onChange={(e) => setFileFilter(e.target.value)}
              />
            </div>
          )}

          {message && (
            <div className="progress-container">
              <p className="message">{message}</p>
              {indexing && progress > 0 && (
                <div className="progress-bar">
                  <div className="progress-fill" style={{ width: `${progress}%` }}></div>
                </div>
              )}
            </div>
          )}

          <div className="main-content-area">
            <div className="results spotlight-results">
              {results.length > 0 ? (
                <ul ref={resultListRef}>
                  {results.map((result, i) => (
                    <li 
                      key={i} 
                      className={`result-item ${activeIndex === i ? 'active' : ''}`}
                      onClick={() => openPath(result.path)}
                      onMouseEnter={() => setActiveIndex(i)}
                    >
                      <div className="result-header">
                        <span className="score">{result.score.toFixed(4)}</span>
                        <span className="path">{result.path}</span>
                      </div>
                      <div 
                        className="snippet" 
                        dangerouslySetInnerHTML={{ __html: result.snippet }} 
                      />
                    </li>
                  ))}
                </ul>
              ) : (
                !searching && query && <p className="no-results">No results found.</p>
              )}
            </div>

            {/* Chat Panel - Only visible if there are results */}
            {results.length > 0 && (
              <div className="chat-panel">
                <h3>Chat with these files</h3>
                <div className="chat-input-group">
                  <input
                    value={chatQuery}
                    onChange={(e) => setChatQuery(e.target.value)}
                    placeholder="Ask a question about these results..."
                    onKeyDown={(e) => e.key === "Enter" && handleChat()}
                  />
                  <button onClick={handleChat} disabled={isChatting || !chatQuery}>
                    {isChatting ? "..." : "Ask"}
                  </button>
                </div>
                {chatResponse && (
                  <div className="chat-response">
                    {chatResponse}
                  </div>
                )}
              </div>
            )}
          </div>
          
          <div className="controls footer-controls">
            <button onClick={handlePickDirectory} disabled={indexing}>
              {indexing ? "Indexing..." : "+ Index Folder"}
            </button>
            {!isExpanded && (
              <button className="compact-settings-btn" onClick={() => setShowSettings(true)}>
                ⚙
              </button>
            )}
            <span className="shortcut-hint">Cmd+Shift+Space to toggle</span>
          </div>
        </>
      )}
    </main>
  );
}

export default App;
