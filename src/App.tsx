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
  const [activeIndex, setActiveIndex] = useState(-1);
  const resultListRef = useRef<HTMLUListElement>(null);
  const [chatQuery, setChatQuery] = useState("");
  const [chatResponse, setChatResponse] = useState("");
  const [isChatting, setIsChatting] = useState(false);
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

  // CRITICAL: Force height based on state
  const isExpanded = results.length > 0 || searching || indexing || showSettings || !!message;

  useEffect(() => {
    const resize = async () => {
      const appWindow = getCurrentWebviewWindow();
      if (isExpanded) {
        await appWindow.setSize(new LogicalSize(750, 550));
      } else {
        await appWindow.setSize(new LogicalSize(750, 180));
      }
    };
    resize();
  }, [isExpanded]);

  useEffect(() => {
    fetchSettings();
    const unlisten = listen<ProgressEvent>("indexing-progress", (event) => {
      setMessage(event.payload.message);
      if (event.payload.total > 0) setProgress((event.payload.current / event.payload.total) * 100);
      if (event.payload.message === "Indexing complete") setTimeout(() => setMessage(""), 3000);
    });
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  useEffect(() => {
    if (settings.theme === "dark") document.body.className = "dark-theme";
    else if (settings.theme === "light") document.body.className = "light-theme";
    else document.body.className = "";
  }, [settings.theme]);

  async function fetchSettings() { try { setSettings(await invoke<Settings>("get_settings")); } catch (e) { console.error(e); } }
  async function saveSettings(newSettings: Settings) { try { await invoke("update_settings", { settings: newSettings }); setSettings(newSettings); } catch (e) { console.error(e); } }
  function handleAddIgnore() { if (newIgnorePath) { saveSettings({ ...settings, ignored_paths: [...settings.ignored_paths, newIgnorePath] }); setNewIgnorePath(""); } }
  function handleRemoveIgnore(p: string) { saveSettings({ ...settings, ignored_paths: settings.ignored_paths.filter(x => x !== p) }); }
  async function handlePickDirectory() {
    const selected = await open({ directory: true, multiple: false, title: "Select Directory" });
    if (selected && typeof selected === "string") { setIndexing(true); setProgress(0); setMessage(`Indexing ${selected}...`); await invoke("index_directory", { dirPath: selected }); setIndexing(false); }
  }
  async function handleSearch() {
    if (!query) return; setSearching(true); setActiveIndex(-1); setChatResponse("");
    try { setResults(await invoke<SearchResult[]>("search", { query, fileTypeFilter: fileFilter || null, isRegex })); } 
    catch (e) { console.error(e); } finally { setSearching(false); }
  }
  async function handleChat() {
    if (!chatQuery || results.length === 0) return; setIsChatting(true); setChatResponse("Thinking...");
    try { setChatResponse(await invoke<string>("ask_question", { query: chatQuery, context: results.slice(0, 5).map(r => r.snippet) })); } 
    catch (e) { setChatResponse(`Error: ${e}`); } finally { setIsChatting(false); }
  }
  function handleKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Enter") { if (activeIndex >= 0 && results[activeIndex]) invoke("open_path", { path: results[activeIndex].path }); else handleSearch(); }
    else if (e.key === "ArrowDown") { e.preventDefault(); setActiveIndex(prev => Math.min(prev + 1, results.length - 1)); }
    else if (e.key === "ArrowUp") { e.preventDefault(); setActiveIndex(prev => Math.max(prev - 1, -1)); }
    else if (e.key === "Escape") { setQuery(""); setResults([]); setActiveIndex(-1); setShowSettings(false); setMessage(""); }
  }

  return (
    <main className={`container ${isExpanded ? 'expanded' : 'compact'}`}>
      {/* SEARCH BAR MUST BE FIRST AND UNCONDITIONALLY RENDERED */}
      <div className="search-bar spotlight-search">
        <input
          autoFocus
          value={query}
          onChange={(e) => { setQuery(e.target.value); setActiveIndex(-1); }}
          placeholder="Search your files..."
          onKeyDown={handleKeyDown}
        />
        <button className="filter-toggle" onClick={() => setShowFilters(!showFilters)}>
          {showFilters ? "▲" : "▼"}
        </button>
        {searching && <div className="spinner"></div>}
      </div>

      {showFilters && !showSettings && (
        <div className="advanced-filters">
          <label><input type="checkbox" checked={isRegex} onChange={(e) => setIsRegex(e.target.checked)} /> Regex</label>
          <input className="ext-filter" placeholder=".rs, .md..." value={fileFilter} onChange={(e) => setFileFilter(e.target.value)} />
        </div>
      )}

      {isExpanded && (
        <div className="expanded-content">
          <div className="header">
            <h1>Semantic Search</h1>
            <button className="settings-toggle" onClick={() => setShowSettings(!showSettings)}>
              {showSettings ? "✕ Close" : "⚙ Settings"}
            </button>
          </div>

          {showSettings ? (
            <div className="settings-panel">
              <div className="setting-group"><label>Ollama URL</label><input value={settings.ollama_url} onChange={(e) => saveSettings({ ...settings, ollama_url: e.target.value })} /></div>
              <div className="setting-group"><label>Theme</label><select value={settings.theme} onChange={(e) => saveSettings({ ...settings, theme: e.target.value })} className="theme-select"><option value="system">System</option><option value="light">Light</option><option value="dark">Dark</option></select></div>
              <div className="setting-group"><label>Ignore List</label><div className="settings-input-group"><input value={newIgnorePath} onChange={(e) => setNewIgnorePath(e.target.value)} onKeyDown={(e) => e.key === "Enter" && handleAddIgnore()} /><button onClick={handleAddIgnore}>Add</button></div>
                <ul className="ignore-list">{settings.ignored_paths.map((p, i) => <li key={i}><span>{p}</span><button className="remove-btn" onClick={() => handleRemoveIgnore(p)}>Remove</button></li>)}</ul>
              </div>
            </div>
          ) : (
            <div className="main-content-area">
              {message && <div className="progress-container"><p className="message">{message}</p>{indexing && <div className="progress-bar"><div className="progress-fill" style={{ width: `${progress}%` }}></div></div>}</div>}
              <div className="results spotlight-results">
                {results.length > 0 ? (
                  <ul ref={resultListRef}>
                    {results.map((r, i) => (
                      <li key={i} className={`result-item ${activeIndex === i ? 'active' : ''}`} onClick={() => invoke("open_path", { path: r.path })} onMouseEnter={() => setActiveIndex(i)}>
                        <div className="result-header"><span className="score">{r.score.toFixed(4)}</span><span className="path">{r.path}</span></div>
                        <div className="snippet" dangerouslySetInnerHTML={{ __html: r.snippet }} />
                      </li>
                    ))}
                  </ul>
                ) : !searching && query && <p className="no-results">No results found.</p>}
              </div>
              {results.length > 0 && (
                <div className="chat-panel">
                  <div className="chat-input-group"><input value={chatQuery} onChange={(e) => setChatQuery(e.target.value)} placeholder="Ask about results..." onKeyDown={(e) => e.key === "Enter" && handleChat()} /><button onClick={handleChat} disabled={isChatting}>{isChatting ? "..." : "Ask"}</button></div>
                  {chatResponse && <div className="chat-response">{chatResponse}</div>}
                </div>
              )}
            </div>
          )}

          <div className="footer-controls">
            <button onClick={handlePickDirectory}>+ Folder</button>
            <span className="shortcut-hint">Cmd+Shift+Space</span>
          </div>
        </div>
      )}
      
      {!isExpanded && (
        <div className="compact-footer">
          <button className="compact-settings-btn" onClick={() => setShowSettings(true)}>⚙ Settings</button>
          <span className="shortcut-hint">Spotlight Mode</span>
        </div>
      )}
    </main>
  );
}

export default App;
