# Math Vision

**VLM-powered Math Sheet Correction**

## Introduction

**Math-Vision** is a command-line-interface for analyzing and correcting hand-written mathematical sheets.

It does this by converting a PDF to an image and sending it to an `ollama` VLM.

After extracting data out of the image, errors and corrections are drawn to an output PDF (a copy of the input PDF).

## Setup

The interface is written in Rust, so you'll need a [Rust Toolchain](https://rust-lang.org).

You should also download the latest [pdfium](https://github.com/bblanchon/pdfium-binaries/releases) library and put it into the "libs" directory (you might need to create the directory if it does not exist).

If the library name does not match the default "/libs/libpdfium.so", 
you can specify the library path via the `--pdfium` argument.

## Command-Line-Interface

```
Interface to Math Vision

Usage: math-vision [OPTIONS] <FILE> <OUTPUT>

Arguments:
  <FILE>    The target PDF file to analyze
  <OUTPUT>  The path where the annotated PDF should be saved to

Options:
      --pdfium <PDFIUM>          The path to the pdfium library [default: ./libs/libpdfium.so]
  -u, --url <URL>                The Ollama Service URL [default: http://localhost:11434]
  -m, --model <MODEL>            The Ollama model to use [default: gemma3:12b]
  -p, --prompt <PROMPT>          The file to read the model prompt from [default: prompt.txt]
  -t, --temp <TEMP>              The model temperature [default: 0.1]
      --ctx <CTX>                The maximum context tokens of the model [default: 8192]
      --predict <PREDICT>        The maximum prediction tokens of the model [default: 8192]
      --top_p <TOP_P>            The top-p model parameter [default: 1.0]
      --thinking <THINKING>      The thinking mode for the model. Can be: 'true', 'false', 'low', 'medium', 'high' [default: false]
      --max-width <MAX_WIDTH>    The maximum width of a rendered PDF page. Rendered pages larger than this, will be resized [default: 2400]
      --max-height <MAX_HEIGHT>  The maximum height of a rendered PDF page. Rendered pages larger than this, will be resized [default: 2400]
  -d, --debug                    Run the application in debug mode
  -h, --help                     Print help
```

# License

This software is licensed under the [ANTI-CHEATER](https://github.com/DraftedDev/Licenses#anti-cheater-license) license.
