use anyhow::{Result, Error};
use candle_core::{Device, Tensor, DType};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config};
use tokenizers::Tokenizer;
use hf_hub::{api::sync::Api, Repo, RepoType};

pub struct EmbeddingModel {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
}

impl EmbeddingModel {
    pub fn new() -> Result<Self> {
        let device = Device::Cpu; 
        
        let api = Api::new()?;
        let repo = api.repo(Repo::new("sentence-transformers/all-MiniLM-L6-v2".to_string(), RepoType::Model));
        
        let model_filename = repo.get("model.safetensors")?;
        let tokenizer_filename = repo.get("tokenizer.json")?;
        let config_filename = repo.get("config.json")?;

        let config = std::fs::read_to_string(config_filename)?;
        let config: Config = serde_json::from_str(&config)?;
        let tokenizer = Tokenizer::from_file(tokenizer_filename).map_err(Error::msg)?;

        let tensors = candle_core::safetensors::load(model_filename, &device)?;
        let vb = VarBuilder::from_tensors(tensors, DType::F32, &device);
        let model = BertModel::load(vb, &config)?;

        Ok(Self {
            model,
            tokenizer,
            device,
        })
    }

    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let tokens = self.tokenizer.encode(text, true).map_err(Error::msg)?;
        let token_ids = tokens.get_ids().to_vec();
        let token_ids = Tensor::new(&token_ids[..], &self.device)?.unsqueeze(0)?;
        let token_type_ids = token_ids.zeros_like()?;
        
        let embeddings = self.model.forward(&token_ids, &token_type_ids, None)?;
        
        // Mean pooling
        let (_n_batch, n_tokens, _hidden_size) = embeddings.dims3()?;
        let embeddings = (embeddings.sum(1)? / (n_tokens as f64))?;
        let embeddings = embeddings.get(0)?;
        
        // Normalize
        let norm = embeddings.sqr()?.sum_all()?.sqrt()?;
        let embeddings = (embeddings / norm)?;
        
        Ok(embeddings.to_vec1()?)
    }
}
