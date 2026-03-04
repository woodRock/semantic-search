import { useState, useEffect } from "react";
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

  // Dynamic window resizing logic
  const isExpanded = results.length > 0 || searching || indexing || showSettings || !!message;

  useEffect(() => {
    const resize = async () => {
      try {
        const appWindow = getCurrentWebviewWindow();
        if (isExpanded) {
          await appWindow.setSize(new LogicalSize(750, 550));
        } else {
          await appWindow.setSize(new LogicalSize(750, 140));
        }
      } catch (e) {
        console.error("Window resize failed:", e);
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

  async function fetchSettings() { try { setSettings(await invoke<Settings>("get_settings")); } catch (e) { console.error(e); } }
  async function saveSettings(newSettings: Settings) { try { await invoke("update_settings", { settings: newSettings }); setSettings(newSettings); } catch (e) { console.error(e); } }

  async function handleSearch() {
    if (!query) return;
    setSearching(true); setChatResponse("");
    try {
      const res = await invoke<SearchResult[]>("search", { 
        query, fileTypeFilter: fileFilter || null, isRegex 
      });
      setResults(res);
    } catch (e) { console.error(e); } finally { setSearching(false); }
  }

  function handleKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Enter") {
      if (results.length > 0) invoke("open_path", { path: results[0].path }); // Open first result
      else handleSearch();
    } else if (e.key === "Escape") {
      setQuery(""); setResults([]); setShowSettings(false); setMessage("");
    }
  }

  return (
    <main className={`container ${isExpanded ? 'expanded' : 'compact'}`}>
      {/* Search bar is always visible */}
      <div className="search-section" style={{ display: 'flex', gap: '10px', alignItems: 'center' }}>
        <input 
          autoFocus
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Search files..."
          style={{ flex: 1, padding: '12px', fontSize: '1.4rem', borderRadius: '10px', border: '1px solid #444', background: '#222', color: 'white' }}
        />
        {!isExpanded && <button onClick={() => setShowSettings(true)} style={{ background: 'transparent', opacity: 0.5 }}>⚙</button>}
      </div>

      {isExpanded && (
        <div className="expanded-view" style={{ flex: 1, overflowY: 'auto', marginTop: '15px' }}>
          <div className="header" style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '15px' }}>
            <span style={{ opacity: 0.5, fontSize: '0.8rem', textTransform: 'uppercase' }}>Semantic Search</span>
            <button onClick={() => setShowSettings(!showSettings)} style={{ fontSize: '0.8rem' }}>{showSettings ? "Close" : "Settings"}</button>
          </div>

          {showSettings ? (
            <div className="settings-panel">
              <div className="setting-group"><label>Ollama URL</label><input value={settings.ollama_url} onChange={(e) => saveSettings({ ...settings, ollama_url: e.target.value })} /></div>
              <div className="setting-group"><label>Ignore List</label>
                <div style={{ display: 'flex', gap: '10px' }}><input value={newIgnorePath} onChange={(e) => setNewIgnorePath(e.target.value)} /><button onClick={() => {saveSettings({...settings, ignored_paths: [...settings.ignored_paths, newIgnorePath]}); setNewIgnorePath("");}}>Add</button></div>
              </div>
            </div>
          ) : (
            <>
              {message && <div style={{ color: '#646cff', fontSize: '0.9rem', marginBottom: '10px' }}>{message}</div>}
              <div className="results-list">
                {results.map((r, i) => (
                  <div key={i} className="result-item" onClick={() => invoke("open_path", { path: r.path })} style={{ padding: '10px', background: '#222', borderRadius: '8px', marginBottom: '8px' }}>
                    <div style={{ fontWeight: 'bold', color: '#646cff', fontSize: '0.9rem' }}>{r.path}</div>
                    <div style={{ fontSize: '0.8rem', color: '#aaa' }} dangerouslySetInnerHTML={{ __html: r.snippet }} />
                  </div>
                ))}
              </div>
              {results.length > 0 && (
                <div className="chat-panel" style={{ marginTop: '10px', padding: '10px', background: '#111', borderRadius: '10px' }}>
                  <div style={{ display: 'flex', gap: '10px' }}>
                    <input value={chatQuery} onChange={(e) => setChatQuery(e.target.value)} placeholder="Ask..." style={{ flex: 1, padding: '5px' }} />
                    <button onClick={async () => {setIsChatting(true); setChatResponse(await invoke("ask_question", { query: chatQuery, context: results.slice(0, 5).map(r => r.snippet) })); setIsChatting(false);}} disabled={isChatting}>Ask</button>
                  </div>
                  {chatResponse && <div style={{ marginTop: '10px', fontSize: '0.85rem', whiteSpace: 'pre-wrap' }}>{chatResponse}</div>}
                </div>
              )}
            </>
          )}
        </div>
      )}

      {isExpanded && (
        <div className="footer" style={{ marginTop: 'auto', paddingTop: '10px', borderTop: '1px solid #333' }}>
          <button onClick={async () => {const s = await open({directory:true}); if(s) await invoke("index_directory", {dirPath: s})}}>+ Folder</button>
        </div>
      )}
    </main>
  );
}

export default App;
