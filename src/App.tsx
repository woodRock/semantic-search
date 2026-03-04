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

function App() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [indexing, setIndexing] = useState(false);
  const [searching, setSearching] = useState(false);
  const [message, setMessage] = useState("");
  const [progress, setProgress] = useState(0);

  useEffect(() => {
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
        // The final progress event sets the "Indexing complete" message.
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
      <h1>Semantic Search</h1>
      
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
    </main>
  );
}

export default App;
