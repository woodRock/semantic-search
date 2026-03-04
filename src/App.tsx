import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import "./App.css";

function App() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<[string, number][]>([]);
  const [indexing, setIndexing] = useState(false);
  const [searching, setSearching] = useState(false);
  const [message, setMessage] = useState("");

  async function handlePickDirectory() {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Select Directory to Index",
      });
      
      if (selected && typeof selected === "string") {
        setIndexing(true);
        setMessage(`Indexing ${selected}...`);
        await invoke("index_directory", { dirPath: selected });
        setMessage("Indexing complete!");
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
      const searchResults = await invoke<[string, number][]>("search", { query });
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

      {message && <p className="message">{message}</p>}

      <div className="results">
        {results.length > 0 ? (
          <ul>
            {results.map(([path, score], i) => (
              <li key={i}>
                <span className="score">{score.toFixed(4)}</span>
                <span className="path">{path}</span>
              </li>
            ))}
          </ul>
        ) : (
          !searching && <p>No results found.</p>
        )}
      </div>
    </main>
  );
}

export default App;
