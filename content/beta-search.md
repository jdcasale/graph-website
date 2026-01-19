# CLAUDE PLACEHOLDER ARTICLE: Project Beta: Search

The search feature uses a custom inverted index for fast full-text search across all your notes.

## How It Works

1. Text is tokenized and normalized
2. Tokens are stored in an inverted index
3. Queries are parsed and matched against the index
4. Results are ranked by relevance

The index is persistent and updates incrementally as you add or modify notes.
