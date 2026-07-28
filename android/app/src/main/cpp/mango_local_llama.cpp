#include <android/log.h>
#include <ggml-backend.h>
#include <jni.h>
#include <llama.h>
#include <unistd.h>

#include <algorithm>
#include <cstdint>
#include <mutex>
#include <string>
#include <vector>

// common (libllama-common.so): embedded chat-template application via Jinja.
// Using common_chat_templates_* lets every model use its own tokenizer.chat_template
// (Llama-3, Gemma, Phi, Qwen...) instead of a hardcoded ChatML formatter.
#include <chat.h>
#include <nlohmann/json.hpp>

#define LOG_TAG "MangoLocalLlama"
#define LOGI(...) __android_log_print(ANDROID_LOG_INFO, LOG_TAG, __VA_ARGS__)
#define LOGW(...) __android_log_print(ANDROID_LOG_WARN, LOG_TAG, __VA_ARGS__)
#define LOGE(...) __android_log_print(ANDROID_LOG_ERROR, LOG_TAG, __VA_ARGS__)

static std::mutex g_mutex;
static bool g_backend_initialized = false;
static llama_model *g_model = nullptr;
static llama_context *g_context = nullptr;
static common_chat_templates *g_chat_templates = nullptr;
static llama_sampler *g_sampler = nullptr;
static llama_batch g_batch;
static bool g_batch_initialized = false;
static int32_t g_batch_capacity = 0;
static int32_t g_current_pos = 0;
static int32_t g_stop_pos = 0;
static bool g_done = true;
static std::string g_cached_utf8;
static std::string g_last_error;

static int android_log_prio_from_ggml(enum ggml_log_level level) {
    switch (level) {
        case GGML_LOG_LEVEL_ERROR: return ANDROID_LOG_ERROR;
        case GGML_LOG_LEVEL_WARN: return ANDROID_LOG_WARN;
        case GGML_LOG_LEVEL_INFO: return ANDROID_LOG_INFO;
        case GGML_LOG_LEVEL_DEBUG: return ANDROID_LOG_DEBUG;
        default: return ANDROID_LOG_DEFAULT;
    }
}

static void llama_android_log(enum ggml_log_level level, const char *text, void *) {
    __android_log_write(android_log_prio_from_ggml(level), LOG_TAG, text);
}

static void set_error(const std::string &message) {
    g_last_error = message;
    LOGE("%s", message.c_str());
}

static void clear_generation_state() {
    g_current_pos = 0;
    g_stop_pos = 0;
    g_done = true;
    g_cached_utf8.clear();
    g_last_error.clear();
}

static void free_model_locked() {
    if (g_chat_templates != nullptr) {
        common_chat_templates_free(g_chat_templates);
        g_chat_templates = nullptr;
    }
    if (g_sampler != nullptr) {
        llama_sampler_free(g_sampler);
        g_sampler = nullptr;
    }
    if (g_batch_initialized) {
        llama_batch_free(g_batch);
        g_batch_initialized = false;
        g_batch_capacity = 0;
    }
    if (g_context != nullptr) {
        llama_free(g_context);
        g_context = nullptr;
    }
    if (g_model != nullptr) {
        llama_model_free(g_model);
        g_model = nullptr;
    }
    clear_generation_state();
}

static bool decode_utf8_to_utf16(const std::string &text, std::vector<jchar> &out) {
    out.clear();
    for (size_t i = 0; i < text.size();) {
        const auto b0 = static_cast<uint8_t>(text[i]);
        uint32_t codepoint = 0;
        size_t needed = 0;
        if ((b0 & 0x80U) == 0) {
            codepoint = b0;
            needed = 1;
        } else if ((b0 & 0xE0U) == 0xC0U) {
            codepoint = b0 & 0x1FU;
            needed = 2;
        } else if ((b0 & 0xF0U) == 0xE0U) {
            codepoint = b0 & 0x0FU;
            needed = 3;
        } else if ((b0 & 0xF8U) == 0xF0U) {
            codepoint = b0 & 0x07U;
            needed = 4;
        } else {
            return false;
        }

        if (i + needed > text.size()) {
            return false;
        }
        for (size_t j = 1; j < needed; ++j) {
            const auto bx = static_cast<uint8_t>(text[i + j]);
            if ((bx & 0xC0U) != 0x80U) {
                return false;
            }
            codepoint = (codepoint << 6U) | (bx & 0x3FU);
        }

        const bool overlong =
            (needed == 2 && codepoint < 0x80U) ||
            (needed == 3 && codepoint < 0x800U) ||
            (needed == 4 && codepoint < 0x10000U);
        const bool surrogate = codepoint >= 0xD800U && codepoint <= 0xDFFFU;
        if (overlong || surrogate || codepoint > 0x10FFFFU) {
            return false;
        }

        if (codepoint <= 0xFFFFU) {
            out.push_back(static_cast<jchar>(codepoint));
        } else {
            codepoint -= 0x10000U;
            out.push_back(static_cast<jchar>(0xD800U + (codepoint >> 10U)));
            out.push_back(static_cast<jchar>(0xDC00U + (codepoint & 0x3FFU)));
        }
        i += needed;
    }
    return true;
}

static jstring new_java_string_utf8(JNIEnv *env, const std::string &text) {
    std::vector<jchar> utf16;
    if (!decode_utf8_to_utf16(text, utf16)) {
        return nullptr;
    }
    return env->NewString(utf16.data(), static_cast<jsize>(utf16.size()));
}

static int tokenize_prompt(const std::string &prompt, std::vector<llama_token> &tokens) {
    const llama_vocab *vocab = llama_model_get_vocab(g_model);
    int32_t needed = llama_tokenize(
        vocab,
        prompt.c_str(),
        static_cast<int32_t>(prompt.size()),
        nullptr,
        0,
        true,
        true
    );
    if (needed == INT32_MIN) {
        set_error("prompt tokenization overflowed");
        return 1;
    }
    if (needed < 0) {
        needed = -needed;
    }
    if (needed == 0) {
        set_error("prompt tokenized to zero tokens");
        return 2;
    }

    tokens.resize(static_cast<size_t>(needed));
    const int32_t actual = llama_tokenize(
        vocab,
        prompt.c_str(),
        static_cast<int32_t>(prompt.size()),
        tokens.data(),
        needed,
        true,
        true
    );
    if (actual < 0) {
        set_error("prompt tokenization failed");
        return 3;
    }
    tokens.resize(static_cast<size_t>(actual));
    return 0;
}

static int decode_tokens_locked(const std::vector<llama_token> &tokens, int32_t start_pos, bool logits_last) {
    for (int32_t offset = 0; offset < static_cast<int32_t>(tokens.size()); offset += g_batch_capacity) {
        const int32_t count = std::min(g_batch_capacity, static_cast<int32_t>(tokens.size()) - offset);
        g_batch.n_tokens = 0;
        for (int32_t i = 0; i < count; ++i) {
            const int32_t index = g_batch.n_tokens++;
            const int32_t absolute = offset + i;
            g_batch.token[index] = tokens[static_cast<size_t>(absolute)];
            g_batch.pos[index] = start_pos + absolute;
            g_batch.n_seq_id[index] = 1;
            g_batch.seq_id[index][0] = 0;
            g_batch.logits[index] = logits_last && absolute == static_cast<int32_t>(tokens.size()) - 1;
        }

        const int32_t result = llama_decode(g_context, g_batch);
        if (result != 0) {
            set_error("llama_decode failed while processing prompt: " + std::to_string(result));
            return 1;
        }
    }
    return 0;
}

static int decode_one_locked(llama_token token) {
    g_batch.n_tokens = 1;
    g_batch.token[0] = token;
    g_batch.pos[0] = g_current_pos;
    g_batch.n_seq_id[0] = 1;
    g_batch.seq_id[0][0] = 0;
    g_batch.logits[0] = 1;
    const int32_t result = llama_decode(g_context, g_batch);
    if (result != 0) {
        set_error("llama_decode failed while generating token: " + std::to_string(result));
        return 1;
    }
    ++g_current_pos;
    return 0;
}

static std::string token_to_piece_locked(llama_token token) {
    const llama_vocab *vocab = llama_model_get_vocab(g_model);
    char small[256];
    int32_t written = llama_token_to_piece(vocab, token, small, sizeof(small), 0, true);
    if (written >= 0) {
        return std::string(small, static_cast<size_t>(written));
    }

    std::string large(static_cast<size_t>(-written), '\0');
    written = llama_token_to_piece(vocab, token, large.data(), static_cast<int32_t>(large.size()), 0, true);
    if (written < 0) {
        set_error("failed to detokenize generated token");
        return "";
    }
    large.resize(static_cast<size_t>(written));
    return large;
}

extern "C" JNIEXPORT void JNICALL
Java_dev_disobey_mango_AndroidLlamaEngine_nativeInit(JNIEnv *env, jobject, jstring native_lib_dir) {
    std::lock_guard<std::mutex> lock(g_mutex);
    if (g_backend_initialized) {
        return;
    }

    llama_log_set(llama_android_log, nullptr);
    const char *dir = env->GetStringUTFChars(native_lib_dir, nullptr);
    if (dir == nullptr) {
        set_error("failed to read native library directory");
        return;
    }
    LOGI("Loading llama.cpp backends from %s", dir);
    ggml_backend_load_all_from_path(dir);
    env->ReleaseStringUTFChars(native_lib_dir, dir);

    llama_backend_init();
    g_backend_initialized = true;
    LOGI("llama.cpp backend initialized: %s", llama_print_system_info());
}

extern "C" JNIEXPORT jint JNICALL
Java_dev_disobey_mango_AndroidLlamaEngine_nativeLoadModel(
    JNIEnv *env,
    jobject,
    jstring model_path,
    jint context_size,
    jint thread_count
) {
    std::lock_guard<std::mutex> lock(g_mutex);
    if (!g_backend_initialized) {
        set_error("llama.cpp backend is not initialized");
        return 1;
    }

    free_model_locked();

    const char *path = env->GetStringUTFChars(model_path, nullptr);
    if (path == nullptr) {
        set_error("failed to read model path");
        return 2;
    }
    llama_model_params model_params = llama_model_default_params();
    model_params.use_mmap = true;
    model_params.use_mlock = false;
    g_model = llama_model_load_from_file(path, model_params);
    env->ReleaseStringUTFChars(model_path, path);
    if (g_model == nullptr) {
        set_error("failed to load GGUF model");
        return 2;
    }

    const int32_t threads = std::max(1, static_cast<int32_t>(thread_count));
    const int32_t n_ctx = std::max(512, static_cast<int32_t>(context_size));
    const int32_t n_batch = 128;

    llama_context_params ctx_params = llama_context_default_params();
    ctx_params.n_ctx = n_ctx;
    ctx_params.n_batch = n_batch;
    ctx_params.n_ubatch = n_batch;
    ctx_params.n_threads = threads;
    ctx_params.n_threads_batch = threads;
    ctx_params.flash_attn_type = LLAMA_FLASH_ATTN_TYPE_DISABLED;

    g_context = llama_init_from_model(g_model, ctx_params);
    if (g_context == nullptr) {
        set_error("failed to initialize llama context");
        free_model_locked();
        return 3;
    }

    g_batch = llama_batch_init(n_batch, 0, 1);
    g_batch_initialized = true;
    g_batch_capacity = n_batch;

    // Load the chat template embedded in the GGUF metadata (tokenizer.chat_template).
    // Passing "" as override defers to the model's own template, so Llama-3/Gemma/Phi/Qwen
    // all format correctly without per-model code. Matches the qvac llama.cpp addon.
    {
        common_chat_templates_ptr tmpl_ptr = common_chat_templates_init(g_model, "");
        g_chat_templates = tmpl_ptr.release();
    }
    if (g_chat_templates == nullptr) {
        set_error("failed to load chat template from model metadata");
        free_model_locked();
        return 4;
    }

    llama_sampler_chain_params sampler_params = llama_sampler_chain_default_params();
    g_sampler = llama_sampler_chain_init(sampler_params);
    llama_sampler_chain_add(g_sampler, llama_sampler_init_top_k(40));
    llama_sampler_chain_add(g_sampler, llama_sampler_init_top_p(0.95f, 1));
    llama_sampler_chain_add(g_sampler, llama_sampler_init_min_p(0.05f, 1));
    llama_sampler_chain_add(g_sampler, llama_sampler_init_temp(0.7f));
    llama_sampler_chain_add(g_sampler, llama_sampler_init_dist(LLAMA_DEFAULT_SEED));

    clear_generation_state();
    LOGI("Loaded llama.cpp model with n_ctx=%d n_batch=%d threads=%d", n_ctx, n_batch, threads);
    return 0;
}

extern "C" JNIEXPORT jint JNICALL
Java_dev_disobey_mango_AndroidLlamaEngine_nativeProcessPrompt(
    JNIEnv *env,
    jobject,
    jstring messages_json,
    jint max_tokens
) {
    std::lock_guard<std::mutex> lock(g_mutex);
    clear_generation_state();
    if (g_model == nullptr || g_context == nullptr || g_sampler == nullptr ||
        g_chat_templates == nullptr || !g_batch_initialized) {
        set_error("model is not loaded");
        return 1;
    }

    const char *raw = env->GetStringUTFChars(messages_json, nullptr);
    if (raw == nullptr) {
        set_error("failed to read prompt messages");
        return 2;
    }
    std::string json_text(raw);
    env->ReleaseStringUTFChars(messages_json, raw);

    // Parse `{"messages":[{"role":"...","content":"..."}, ...]}` and apply the
    // model's embedded chat template via common_chat_templates_apply.
    std::vector<common_chat_msg> messages;
    try {
        auto root = nlohmann::ordered_json::parse(json_text);
        const auto msgs = root.value("messages", nlohmann::ordered_json::array());
        for (const auto &item : msgs) {
            common_chat_msg msg;
            msg.role = item.value("role", "user");
            msg.content = item.value("content", "");
            if (!msg.content.empty()) {
                messages.push_back(std::move(msg));
            }
        }
    } catch (const std::exception &e) {
        set_error(std::string("failed to parse messages JSON: ") + e.what());
        return 2;
    }
    if (messages.empty()) {
        common_chat_msg fallback;
        fallback.role = "user";
        fallback.content = "Reply to the user.";
        messages.push_back(std::move(fallback));
    }

    common_chat_templates_inputs inputs;
    inputs.messages = std::move(messages);
    inputs.add_generation_prompt = true;
    inputs.use_jinja = true;
    // Mango streams a short final answer and does not expose a separate reasoning
    // channel. Pass the policy into the model's own Jinja template: templates that
    // support thinking (for example Qwen 3/3.5) emit their non-thinking prefill,
    // while templates without that capability simply ignore the value.
    inputs.enable_thinking = false;

    std::string prompt_text;
    try {
        auto formatted = common_chat_templates_apply(g_chat_templates, inputs);
        prompt_text = std::move(formatted.prompt);
        LOGI("Applied embedded chat template thinking=%s",
             formatted.supports_thinking ? "disabled" : "not-supported");
    } catch (const std::exception &e) {
        set_error(std::string("chat template application failed: ") + e.what());
        return 2;
    }

    std::vector<llama_token> tokens;
    if (tokenize_prompt(prompt_text, tokens) != 0) {
        return 2;
    }

    const int32_t n_ctx = static_cast<int32_t>(llama_n_ctx(g_context));
    const int32_t budget = std::max(1, static_cast<int32_t>(max_tokens));
    const int32_t max_prompt_tokens = std::max(1, n_ctx - budget - 8);
    if (static_cast<int32_t>(tokens.size()) > max_prompt_tokens) {
        LOGW("Prompt has %d tokens; keeping last %d for local context",
             static_cast<int32_t>(tokens.size()), max_prompt_tokens);
        tokens.erase(tokens.begin(), tokens.end() - max_prompt_tokens);
    }

    llama_memory_clear(llama_get_memory(g_context), false);
    llama_sampler_reset(g_sampler);

    if (decode_tokens_locked(tokens, 0, true) != 0) {
        return 3;
    }

    g_current_pos = static_cast<int32_t>(tokens.size());
    g_stop_pos = g_current_pos + budget;
    g_done = false;
    LOGI("Processed local prompt tokens=%d max_tokens=%d", g_current_pos, budget);
    return 0;
}

extern "C" JNIEXPORT jstring JNICALL
Java_dev_disobey_mango_AndroidLlamaEngine_nativeNextToken(JNIEnv *env, jobject) {
    std::lock_guard<std::mutex> lock(g_mutex);
    if (g_done || g_context == nullptr || g_sampler == nullptr) {
        return nullptr;
    }
    if (g_current_pos >= g_stop_pos) {
        g_done = true;
        return nullptr;
    }

    const llama_token token = llama_sampler_sample(g_sampler, g_context, -1);
    llama_sampler_accept(g_sampler, token);
    const llama_vocab *vocab = llama_model_get_vocab(g_model);
    if (llama_vocab_is_eog(vocab, token)) {
        g_done = true;
        return nullptr;
    }

    if (decode_one_locked(token) != 0) {
        g_done = true;
        return nullptr;
    }

    g_cached_utf8 += token_to_piece_locked(token);
    if (!g_last_error.empty()) {
        g_done = true;
        return nullptr;
    }
    std::vector<jchar> utf16_probe;
    if (!decode_utf8_to_utf16(g_cached_utf8, utf16_probe)) {
        return env->NewStringUTF("");
    }

    const std::string output = g_cached_utf8;
    g_cached_utf8.clear();
    return new_java_string_utf8(env, output);
}

extern "C" JNIEXPORT jstring JNICALL
Java_dev_disobey_mango_AndroidLlamaEngine_nativeLastError(JNIEnv *env, jobject) {
    std::lock_guard<std::mutex> lock(g_mutex);
    if (g_last_error.empty()) {
        return nullptr;
    }
    return env->NewStringUTF(g_last_error.c_str());
}

extern "C" JNIEXPORT void JNICALL
Java_dev_disobey_mango_AndroidLlamaEngine_nativeUnload(JNIEnv *, jobject) {
    std::lock_guard<std::mutex> lock(g_mutex);
    free_model_locked();
}
