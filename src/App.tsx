import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
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

  useEffect(() => {
    fetchSettings();
    const unlisten = listen<ProgressEvent>("indexing-progress", (event) => {
      setMessage(event.payload.message);
      if (event.payload.total > 0) setProgress((event.payload.current / event.payload.total) * 100);
    });
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  // Theme effect
  useEffect(() => {
    const root = document.documentElement;
    if (settings.theme === "dark") {
      root.classList.add("dark");
      root.classList.remove("light");
    } else if (settings.theme === "light") {
      root.classList.add("light");
      root.classList.remove("dark");
    } else {
      root.classList.remove("dark", "light");
    }
  }, [settings.theme]);

  async function fetchSettings() {
    try { setSettings(await invoke<Settings>("get_settings")); } catch (e) { console.error(e); }
  }

  async function saveSettings(newSettings: Settings) {
    try {
      await invoke("update_settings", { settings: newSettings });
      setSettings(newSettings);
    } catch (e) { console.error(e); }
  }

  async function handleSearch() {
    if (!query) return;
    setSearching(true);
    setChatResponse("");
    try {
      const res = await invoke<SearchResult[]>("search", { 
        query, fileTypeFilter: fileFilter || null, isRegex 
      });
      setResults(res);
    } catch (e) { console.error(e); } finally { setSearching(false); }
  }

  return (
    <main className="app-container">
      {/* Settings Bar / Navigation */}
      <nav className="navbar">
        <div className="nav-left">
          <span className="logo">🔍 Semantic Search</span>
        </div>
        <div className="nav-right">
          <button className="nav-btn" onClick={() => setShowSettings(!showSettings)}>
            {showSettings ? "✕ Close" : "⚙ Settings"}
          </button>
        </div>
      </nav>

      <div className="content">
        {showSettings ? (
          <div className="settings-panel">
            <h2>Preferences</h2>
            <div className="setting-group">
              <label>Ollama API URL</label>
              <input value={settings.ollama_url} onChange={(e) => saveSettings({ ...settings, ollama_url: e.target.value })} />
            </div>
            <div className="setting-group">
              <label>Theme Mode</label>
              <select value={settings.theme} onChange={(e) => saveSettings({ ...settings, theme: e.target.value })}>
                <option value="system">System Default</option>
                <option value="light">Light Mode</option>
                <option value="dark">Dark Mode</option>
              </select>
            </div>
            <div className="setting-group">
              <label>Ignore List</label>
              <div className="input-row">
                <input value={newIgnorePath} onChange={(e) => setNewIgnorePath(e.target.value)} placeholder="e.g. node_modules" />
                <button onClick={() => {saveSettings({...settings, ignored_paths: [...settings.ignored_paths, newIgnorePath]}); setNewIgnorePath("");}}>Add</button>
              </div>
              <ul className="ignore-list">
                {settings.ignored_paths.map(p => (
                  <li key={p}>{p} <button onClick={() => saveSettings({...settings, ignored_paths: settings.ignored_paths.filter(x => x !== p)})}>x</button></li>
                ))}
              </ul>
            </div>
          </div>
        ) : (
          <div className="search-view">
            <div className="search-bar-container">
              <div className="search-input-wrapper">
                <input 
                  autoFocus
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && handleSearch()}
                  placeholder="Ask anything about your files..."
                />
                <button className="search-btn" onClick={handleSearch} disabled={searching}>
                  {searching ? "Searching..." : "Search"}
                </button>
              </div>
              <div className="search-options">
                <label><input type="checkbox" checked={isRegex} onChange={(e) => setIsRegex(e.target.checked)} /> Regex</label>
                <input className="ext-input" placeholder="Extension (e.g. .md)" value={fileFilter} onChange={(e) => setFileFilter(e.target.value)} />
              </div>
            </div>

            {message && (
              <div className="status-msg">
                {message}
                {indexing && <div className="progress-bg"><div className="progress-fill" style={{ width: `${progress}%` }} /></div>}
              </div>
            )}

            <div className="results-container">
              <div className="results-list">
                {results.map((r, i) => (
                  <div key={i} className="result-card" onClick={() => invoke("open_path", { path: r.path })}>
                    <div className="res-path">{r.path}</div>
                    <div className="res-snippet" dangerouslySetInnerHTML={{ __html: r.snippet }} />
                  </div>
                ))}
                {!searching && results.length === 0 && query && <p className="empty-state">No results found.</p>}
              </div>

              {results.length > 0 && (
                <div className="chat-sidebar">
                  <h3>Ask AI about these results</h3>
                  <div className="chat-input-row">
                    <input value={chatQuery} onChange={(e) => setChatQuery(e.target.value)} placeholder="What's in these files?" />
                    <button onClick={async () => {setIsChatting(true); setChatResponse(await invoke("ask_question", { query: chatQuery, context: results.slice(0, 5).map(r => r.snippet) })); setIsChatting(false);}} disabled={isChatting}>Ask</button>
                  </div>
                  {chatResponse && <div className="chat-output">{chatResponse}</div>}
                </div>
              )}
            </div>
          </div>
        )}
      </div>

      <footer className="app-footer">
        <button className="index-btn" onClick={async () => {const s = await open({directory:true}); if(s) {setIndexing(true); await invoke("index_directory", {dirPath: s}); setIndexing(false);}}}>
          📁 Index New Directory
        </button>
        <div className="hint">Press Enter to search, Cmd+O to open results</div>
      </footer>
    </main>
  );
}

export default App;
