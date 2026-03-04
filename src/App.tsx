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

  const isExpanded = results.length > 0 || searching || indexing || showSettings || !!message;

  useEffect(() => {
    const resize = async () => {
      try {
        const appWindow = getCurrentWebviewWindow();
        if (isExpanded) {
          await appWindow.setSize(new LogicalSize(750, 550));
        } else {
          await appWindow.setSize(new LogicalSize(750, 100));
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
    });
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  async function fetchSettings() { try { setSettings(await invoke<Settings>("get_settings")); } catch (e) { console.error(e); } }
  async function saveSettings(newSettings: Settings) { try { await invoke("update_settings", { settings: newSettings }); setSettings(newSettings); } catch (e) { console.error(e); } }

  async function handleSearch() {
    if (!query) return;
    setSearching(true);
    try {
      const res = await invoke<SearchResult[]>("search", { 
        query, fileTypeFilter: fileFilter || null, isRegex 
      });
      setResults(res);
    } catch (e) { console.error(e); } finally { setSearching(false); }
  }

  return (
    <main className={`container ${isExpanded ? 'expanded' : 'compact'}`}>
      {isExpanded && (
        <div className="header">
          <h1>Semantic Search</h1>
          <button onClick={() => setShowSettings(!showSettings)}>{showSettings ? "Back" : "Settings"}</button>
        </div>
      )}

      <div className="search-view">
        <div className="search-controls">
          <input 
            autoFocus
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleSearch()}
            placeholder="Search your files..."
          />
          {!isExpanded && <button onClick={() => setShowSettings(true)} style={{ background: 'transparent', opacity: 0.5 }}>⚙</button>}
        </div>

        {isExpanded && (
          <div className="expanded-view">
            {showSettings ? (
              <div className="settings-panel">
                <div className="setting-group"><label>Ollama URL</label><input value={settings.ollama_url} onChange={(e) => saveSettings({ ...settings, ollama_url: e.target.value })} /></div>
                <div className="setting-group"><label>Ignore List</label>
                  <div style={{ display: 'flex', gap: '10px' }}><input value={newIgnorePath} onChange={(e) => setNewIgnorePath(e.target.value)} placeholder="Pattern..."/><button onClick={() => {saveSettings({...settings, ignored_paths: [...settings.ignored_paths, newIgnorePath]}); setNewIgnorePath("");}}>Add</button></div>
                </div>
              </div>
            ) : (
              <>
                <div className="filters">
                  <label><input type="checkbox" checked={isRegex} onChange={(e) => setIsRegex(e.target.checked)} /> Regex</label>
                  <input placeholder="Ext" value={fileFilter} onChange={(e) => setFileFilter(e.target.value)} style={{ width: '60px', marginLeft: '10px' }} />
                </div>
                {message && <div className="progress"><div>{message}</div>{indexing && <div className="p-bar"><div style={{ width: `${progress}%` }} /></div>}</div>}
                <div className="results-list">
                  {results.map((r, i) => (
                    <div key={i} className="result-item" onClick={() => invoke("open_path", { path: r.path })}>
                      <div className="path">{r.path}</div>
                      <div className="snippet" dangerouslySetInnerHTML={{ __html: r.snippet }} />
                    </div>
                  ))}
                </div>
                {results.length > 0 && (
                  <div className="chat-area">
                    <div className="chat-input"><input value={chatQuery} onChange={(e) => setChatQuery(e.target.value)} placeholder="Ask..." /><button onClick={async () => {setIsChatting(true); setChatResponse(await invoke("ask_question", { query: chatQuery, context: results.slice(0, 5).map(r => r.snippet) })); setIsChatting(false);}} disabled={isChatting}>Ask</button></div>
                    {chatResponse && <div className="chat-response">{chatResponse}</div>}
                  </div>
                )}
              </>
            )}
          </div>
        )}
      </div>

      {isExpanded && (
        <div className="footer">
          <button onClick={async () => {const s = await open({ directory: true }); if(s) {setIndexing(true); await invoke("index_directory", {dirPath: s}); setIndexing(false);}}}>+ Folder</button>
        </div>
      )}
    </main>
  );
}

export default App;
