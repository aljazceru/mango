cmake_minimum_required(VERSION 3.22.1)

project(mango_local_llama LANGUAGES C CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

set(LLAMA_CPP_DIR "" CACHE PATH "Path to llama.cpp checkout")
if(NOT LLAMA_CPP_DIR AND DEFINED ENV{LLAMA_CPP_DIR})
    set(LLAMA_CPP_DIR "$ENV{LLAMA_CPP_DIR}")
endif()
if(NOT LLAMA_CPP_DIR)
    message(FATAL_ERROR "LLAMA_CPP_DIR is required. Set LLAMA_CPP_DIR or Gradle -PllamaCppDir to a llama.cpp checkout.")
endif()
if(NOT EXISTS "${LLAMA_CPP_DIR}/include/llama.h")
    message(FATAL_ERROR "llama.cpp header not found at ${LLAMA_CPP_DIR}/include/llama.h")
endif()
if(NOT EXISTS "${LLAMA_CPP_DIR}/ggml/include/ggml.h")
    message(FATAL_ERROR "ggml header not found at ${LLAMA_CPP_DIR}/ggml/include/ggml.h")
endif()

set(LLAMA_PREBUILT_DIR "${CMAKE_CURRENT_LIST_DIR}/../jniLibs/${ANDROID_ABI}" CACHE PATH "Path to prebuilt llama.cpp Android libraries")

foreach(lib_name ggml-base ggml-cpu ggml llama llama-common)
    if(NOT EXISTS "${LLAMA_PREBUILT_DIR}/lib${lib_name}.so")
        message(FATAL_ERROR "Missing prebuilt llama.cpp library: ${LLAMA_PREBUILT_DIR}/lib${lib_name}.so")
    endif()
    add_library(${lib_name} SHARED IMPORTED GLOBAL)
    set_target_properties(${lib_name} PROPERTIES
        IMPORTED_LOCATION "${LLAMA_PREBUILT_DIR}/lib${lib_name}.so"
    )
endforeach()

add_library(mango_local_llama SHARED
    mango_local_llama.cpp
)

target_compile_definitions(mango_local_llama PRIVATE
    LLAMA_SHARED
)

target_include_directories(mango_local_llama PRIVATE
    "${LLAMA_CPP_DIR}/include"
    "${LLAMA_CPP_DIR}/ggml/include"
    "${LLAMA_CPP_DIR}/common"
    "${LLAMA_CPP_DIR}/vendor"
)

target_link_libraries(mango_local_llama
    llama-common
    llama
    ggml
    ggml-base
    ggml-cpu
    android
    log
)
