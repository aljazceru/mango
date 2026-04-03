package dev.disobey.mango

import com.google.gson.Gson

data class TokenizerOutput(
    val inputIds: LongArray,
    val attentionMask: LongArray,
    val tokenTypeIds: LongArray
)

class WordPieceTokenizer(tokenizerJson: String) {
    private val vocab: Map<String, Int>
    private val unkTokenId = 100  // [UNK]
    private val clsTokenId = 101  // [CLS]
    private val sepTokenId = 102  // [SEP]

    init {
        // Parse tokenizer.json: model.vocab is Map<String, Int>
        data class TokenizerModel(val vocab: Map<String, Int>)
        data class TokenizerJson(val model: TokenizerModel)
        val parsed = Gson().fromJson(tokenizerJson, TokenizerJson::class.java)
        vocab = parsed.model.vocab
        require(vocab.isNotEmpty()) { "Vocabulary is empty" }
    }

    fun tokenize(text: String, maxLength: Int = 256): TokenizerOutput {
        // 1. Lowercase (uncased model)
        val lowered = text.lowercase()

        // 2. Basic tokenization: split on whitespace and punctuation.
        //    Insert spaces around punctuation/symbol chars, then split on whitespace.
        val cleaned = buildString {
            for (ch in lowered) {
                when {
                    ch.isLetterOrDigit() || ch == '\'' -> append(ch)
                    ch.isWhitespace() -> append(' ')
                    else -> append(" $ch ")  // punctuation and other symbols
                }
            }
        }
        val words = cleaned.split(" ").filter { it.isNotEmpty() }

        // 3. WordPiece: greedy longest-match for each word
        val tokenIds = mutableListOf<Int>()
        val maxTokens = maxLength - 2  // Reserve for [CLS] and [SEP]

        for (word in words) {
            if (tokenIds.size >= maxTokens) break
            val wordTokens = wordPiece(word)
            for (t in wordTokens) {
                if (tokenIds.size >= maxTokens) break
                tokenIds.add(t)
            }
        }

        // 4. Assemble output with special tokens: [CLS] ... tokens ... [SEP] [PAD]...
        val inputIds = LongArray(maxLength)        // default 0L = [PAD]
        val attentionMask = LongArray(maxLength)   // default 0L = padding position
        val tokenTypeIds = LongArray(maxLength)    // all zeros (single-sentence input)

        // [CLS] at position 0
        inputIds[0] = clsTokenId.toLong()
        attentionMask[0] = 1L

        // Token positions 1..tokenIds.size
        for (i in tokenIds.indices) {
            inputIds[i + 1] = tokenIds[i].toLong()
            attentionMask[i + 1] = 1L
        }

        // [SEP] after tokens
        val sepPos = tokenIds.size + 1
        inputIds[sepPos] = sepTokenId.toLong()
        attentionMask[sepPos] = 1L

        // Remaining positions left as 0 (PAD token id, zero attention mask)

        return TokenizerOutput(inputIds, attentionMask, tokenTypeIds)
    }

    // Greedy longest-match-first WordPiece sub-word tokenizer.
    // Matches the longest vocabulary entry starting at each position.
    // Uses "##" prefix for continuation sub-words (non-start positions).
    private fun wordPiece(word: String): List<Int> {
        val tokens = mutableListOf<Int>()
        var start = 0
        while (start < word.length) {
            var end = word.length
            var found = false
            while (start < end) {
                val substr = if (start == 0) {
                    word.substring(start, end)
                } else {
                    "##" + word.substring(start, end)
                }
                val id = vocab[substr]
                if (id != null) {
                    tokens.add(id)
                    start = end
                    found = true
                    break
                }
                end--
            }
            if (!found) {
                // Character not in vocab at all; emit [UNK] and advance one character
                tokens.add(unkTokenId)
                start++
            }
        }
        return tokens
    }
}
