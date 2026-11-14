"""PyInstaller entry point. Must use an absolute import: the frozen app runs this
file as __main__, where relative imports fail."""

from ship_provisions.app import main

if __name__ == "__main__":
    main()
