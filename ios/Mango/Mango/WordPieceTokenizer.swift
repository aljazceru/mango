import Foundation

// MARK: - TokenizerOutput

struct TokenizerOutput {
    let inputIds: [Int64]
    let attentionMask: [Int64]
    let tokenTypeIds: [Int64]
}

// MARK: - EmbeddingError

enum EmbeddingError: Error {
    case modelNotFound
    case tokenizerNotFound
    case vocabEmpty
    case sessionCreationFailed(String)
}

// MARK: - WordPieceTokenizer

/// WordPiece tokenizer for all-MiniLM-L6-v2 (uncased BERT variant).
///
/// Loads vocabulary from tokenizer.json (HuggingFace tokenizers format).
/// Implements greedy longest-match-first WordPiece segmentation.
/// Produces input_ids, attention_mask, and token_type_ids tensors
/// suitable for ONNX Runtime inference.
final class WordPieceTokenizer {
    private let vocab: [String: Int]
    // Special token IDs for BERT uncased vocabulary
    private let unkTokenId: Int = 100   // [UNK]
    private let clsTokenId: Int = 101   // [CLS]
    private let sepTokenId: Int = 102   // [SEP]
    private let padTokenId: Int = 0     // [PAD]

    /// Initialize from tokenizer.json in HuggingFace tokenizers format.
    /// Expected structure: { "model": { "vocab": { "[PAD]": 0, ... } } }
    init(tokenizerURL: URL) throws {
        let data = try Data(contentsOf: tokenizerURL)

        struct Model: Codable {
            let vocab: [String: Int]
        }
        struct TokenizerJSON: Codable {
            let model: Model
        }

        let decoded = try JSONDecoder().decode(TokenizerJSON.self, from: data)
        self.vocab = decoded.model.vocab
        guard !vocab.isEmpty else { throw EmbeddingError.vocabEmpty }
    }

    /// Tokenize a single text string.
    ///
    /// - Parameters:
    ///   - text: Input text to tokenize.
    ///   - maxLength: Total sequence length including [CLS] and [SEP] tokens. Defaults to 256.
    /// - Returns: TokenizerOutput with inputIds, attentionMask, tokenTypeIds all of length maxLength.
    func tokenize(_ text: String, maxLength: Int = 256) -> TokenizerOutput {
        // Step 1: Lowercase (uncased model requires lowercase normalization)
        let lowered = text.lowercased()

        // Step 2: Basic tokenization — insert spaces around punctuation and symbols,
        // then split on whitespace to produce word tokens.
        var cleaned = ""
        cleaned.reserveCapacity(lowered.count * 2)
        for char in lowered {
            if char.isPunctuation || char.isSymbol {
                cleaned += " \(char) "
            } else {
                cleaned.append(char)
            }
        }
        let words = cleaned.split(separator: " ").map(String.init).filter { !$0.isEmpty }

        // Step 3: WordPiece segmentation — greedy longest-match-first for each word.
        // Reserve 2 positions for [CLS] and [SEP] special tokens.
        var tokenIds: [Int] = []
        let maxTokens = maxLength - 2

        for word in words {
            guard tokenIds.count < maxTokens else { break }
            let wordTokens = wordPiece(word)
            for t in wordTokens {
                guard tokenIds.count < maxTokens else { break }
                tokenIds.append(t)
            }
        }

        // Step 4: Prepend [CLS] (101) and append [SEP] (102)
        var inputIds: [Int64] = [Int64(clsTokenId)]
        inputIds.append(contentsOf: tokenIds.map { Int64($0) })
        inputIds.append(Int64(sepTokenId))

        // Step 5: Attention mask — 1 for real tokens, 0 for padding
        let realLen = inputIds.count
        var attentionMask = [Int64](repeating: 1, count: realLen)

        // Step 6: Pad to maxLength with [PAD] (0)
        while inputIds.count < maxLength {
            inputIds.append(Int64(padTokenId))
            attentionMask.append(0)
        }

        // Step 7: token_type_ids — all zeros for single-sentence input
        let tokenTypeIds = [Int64](repeating: 0, count: maxLength)

        return TokenizerOutput(
            inputIds: inputIds,
            attentionMask: attentionMask,
            tokenTypeIds: tokenTypeIds
        )
    }

    // MARK: - WordPiece Algorithm

    /// Greedy longest-match-first WordPiece segmentation.
    ///
    /// For each word, tries to match the longest vocabulary prefix.
    /// Continuation subwords are prefixed with "##" (BERT convention).
    /// Returns [unkTokenId] for any character that cannot be matched.
    private func wordPiece(_ word: String) -> [Int] {
        var tokens: [Int] = []
        var start = word.startIndex

        while start < word.endIndex {
            var end = word.endIndex
            var found = false

            // Try longest match first, shrinking end index until a vocab entry is found
            while start < end {
                let substr: String
                if start == word.startIndex {
                    // First subword: no prefix
                    substr = String(word[start..<end])
                } else {
                    // Continuation subword: "##" prefix per BERT WordPiece convention
                    substr = "##" + String(word[start..<end])
                }

                if let id = vocab[substr] {
                    tokens.append(id)
                    start = end
                    found = true
                    break
                }

                // Shrink match window by one character
                end = word.index(before: end)
            }

            if !found {
                // Cannot match any prefix — emit [UNK] and advance one character
                tokens.append(unkTokenId)
                start = word.index(after: start)
            }
        }

        return tokens
    }
}
