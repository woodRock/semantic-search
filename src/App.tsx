import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import "./App.css";

interface SearchResult {
  path: string;
  score: number;
  snippet: string;
  summary: string;
  modified: number;
}

interface ProgressEvent {
  message: string;
  current: number;
  total: number;
}

interface Settings {
  ignored_paths: string[];
  ollama_url: string;
  model_name: string;
  theme: string;
}

function App() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  
  const [indexing, setIndexing] = useState(false);
  const [searching, setSearching] = useState(false);
  const [message, setMessage] = useState("");
  const [progress, setProgress] = useState(0);
  
  const [limit, setLimit] = useState(20);
  const [sortOrder, setSortOrder] = useState<"relevance" | "date">("relevance");

  const [chatQuery, setChatQuery] = useState("");
  const [chatResponse, setChatResponse] = useState("");
  const [isChatting, setIsChatting] = useState(false);
  const [showChat, setShowChat] = useState(false);

  const [showSettings, setShowSettings] = useState(false);
  const [settings, setSettings] = useState<Settings>({ 
    ignored_paths: [], 
    ollama_url: "http://localhost:11434",
    model_name: "qwen3.5:0.8b",
    theme: "system"
  });

  const [newIgnorePath, setNewIgnorePath] = useState("");

  useEffect(() => {
    fetchSettings();
    const unlistenProgress = listen<ProgressEvent>("indexing-progress", (event) => {
      setMessage(event.payload.message);
      if (event.payload.total > 0) setProgress((event.payload.current / event.payload.total) * 100);
    });
    
    return () => { 
      unlistenProgress.then((fn) => fn()); 
    };
  }, []);

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
      const res = await invoke<SearchResult[]>("simple_search", { 
        query, 
        limit
      });
      setResults(res);
    } catch (e) { 
      console.error(e); 
      setMessage("Error performing search. Is Ollama running?");
    } finally { 
      setSearching(false); 
    }
  }

  const sortedResults = [...results].sort((a, b) => {
    if (sortOrder === "date") return b.modified - a.modified;
    return 0; // Already sorted by relevance from backend
  });

  return (
    <main className="app-container">
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
              <label>Model Name</label>
              <input value={settings.model_name} onChange={(e) => saveSettings({ ...settings, model_name: e.target.value })} placeholder="e.g. qwen3.5:0.8b" />
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
                <button className="tool-btn" onClick={() => {saveSettings({...settings, ignored_paths: [...settings.ignored_paths, newIgnorePath]}); setNewIgnorePath("");}}>Add</button>
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
                  placeholder="Search for keywords or describe what you're looking for..."
                />
                <button className="search-btn" onClick={handleSearch} disabled={searching}>
                  {searching ? "Searching..." : "Search"}
                </button>
              </div>
              <div className="search-options" style={{justifyContent: 'flex-end'}}>
                <select className="tool-select" value={limit} onChange={(e) => setLimit(Number(e.target.value))}>
                  <option value={10}>Top 10</option>
                  <option value={20}>Top 20</option>
                  <option value={50}>Top 50</option>
                </select>
                <select className="tool-select" value={sortOrder} onChange={(e) => setSortOrder(e.target.value as "relevance" | "date")}>
                  <option value="relevance">Sort by Relevance</option>
                  <option value="date">Sort by Date</option>
                </select>
              </div>
            </div>

            {message && !searching && (
              <div className="status-msg">
                {message}
                {indexing && <div className="progress-bg"><div className="progress-fill" style={{ width: `${progress}%` }} /></div>}
              </div>
            )}

            <div className="results-container">
              {results.length > 0 && (
                <div className="results-header">
                  <span>Found {results.length} results</span>
                  <button className="ask-ai-btn" onClick={() => setShowChat(!showChat)}>
                    ✨ Chat with results
                  </button>
                </div>
              )}
              
              <div className="results-list">
                {sortedResults.map((r, i) => (
                  <div key={i} className="result-card" onClick={() => invoke("open_path", { path: r.path })}>
                    <div className="res-header-row">
                      <div className="res-path">{r.path}</div>
                      <div className="res-date">{r.modified > 0 ? new Date(r.modified * 1000).toLocaleDateString() : ""}</div>
                    </div>
                    {r.summary && r.summary !== "No summary available." && <div className="res-summary">{r.summary}</div>}
                    <div className="res-snippet" dangerouslySetInnerHTML={{ __html: r.snippet }} />
                  </div>
                ))}
                {!searching && results.length === 0 && query && <p className="empty-state">No results found.</p>}
              </div>

              {showChat && (
                <div className="chat-overlay">
                  <div className="chat-header">
                    <h3>AI Assistant</h3>
                    <button className="close-btn" style={{color:'white'}} onClick={() => setShowChat(false)}>✕</button>
                  </div>
                  <div className="chat-body">
                    <div className="chat-output">
                      {isChatting ? "Thinking..." : chatResponse || "Ask a question about the current search results."}
                    </div>
                    <div className="chat-input-row">
                      <input 
                        value={chatQuery} 
                        onChange={(e) => setChatQuery(e.target.value)} 
                        onKeyDown={(e) => e.key === "Enter" && !isChatting && (async () => {
                          setIsChatting(true); 
                          const res = await invoke<string>("ask_question", { 
                            query: chatQuery, 
                            context: results.slice(0, 5).map(r => r.summary + "\n" + r.snippet) 
                          });
                          setChatResponse(res);
                          setIsChatting(false);
                        })()}
                        placeholder="e.g. Which of these is most relevant to X?" 
                      />
                    </div>
                  </div>
                </div>
              )}
            </div>
          </div>
        )}
      </div>

      <footer className="app-footer">
        <button className="index-btn" onClick={async () => {
          const s = await open({directory:true}); 
          if(s) {
            setIndexing(true); 
            try {
              await invoke("index_directory", {dirPath: s});
            } catch (e) {
              console.error(e);
              setMessage("Error during indexing");
            } finally {
              setIndexing(false);
            }
          }
        }}>
          📁 Index New Directory
        </button>
        <div className="hint">Hybrid search: semantic embeddings + keyword matching</div>
      </footer>
    </main>
  );
}

export default App;
