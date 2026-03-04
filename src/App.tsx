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
}

function App() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [indexing, setIndexing] = useState(false);
  const [searching, setSearching] = useState(false);
  const [message, setMessage] = useState("");
  const [progress, setProgress] = useState(0);
  
  // Settings state
  const [showSettings, setShowSettings] = useState(false);
  const [settings, setSettings] = useState<Settings>({ ignored_paths: [] });
  const [newIgnorePath, setNewIgnorePath] = useState("");

  useEffect(() => {
    // Fetch settings on mount
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
    try {
      const searchResults = await invoke<SearchResult[]>("search", { query });
      setResults(searchResults);
    } catch (e) {
      console.error(e);
      setMessage(`Error: ${e}`);
    } finally {
      setSearching(false);
    }
  }

  return (
    <main className="container">
      <div className="header">
        <h1>Semantic Search</h1>
        <button className="settings-toggle" onClick={() => setShowSettings(!showSettings)}>
          {showSettings ? "✕ Close Settings" : "⚙ Settings"}
        </button>
      </div>
      
      {showSettings ? (
        <div className="settings-panel">
          <h2>Ignore List</h2>
          <p className="description">Files or directories matching these patterns will be skipped during indexing (e.g., "node_modules", ".git").</p>
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
      ) : (
        <>
          <div className="controls">
            <button onClick={handlePickDirectory} disabled={indexing}>
              {indexing ? "Indexing..." : "Index Directory"}
            </button>
          </div>

          <div className="search-bar">
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Enter search query..."
              onKeyDown={(e) => e.key === "Enter" && handleSearch()}
            />
            <button onClick={handleSearch} disabled={searching}>
              {searching ? "Searching..." : "Search"}
            </button>
          </div>

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

          <div className="results">
            {results.length > 0 ? (
              <ul>
                {results.map((result, i) => (
                  <li key={i} className="result-item">
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
              !searching && query && <p>No results found.</p>
            )}
          </div>
        </>
      )}
    </main>
  );
}

export default App;
