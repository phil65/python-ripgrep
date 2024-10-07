from python_ripgrep import find_files, search


def main():
    pattern = "test"
    path = "."
    results = search(pattern, path, file_pattern="*.py")
    print(f"Search results for '{pattern}' in '{path}':")
    for match in results:
        print(match)

    # Test find_files function
    file_pattern = r".*"
    path = "."
    files = find_files(file_pattern, path, ["**/*.pyi"])
    print(f"Files matching pattern '{file_pattern}' in '{path}':")
    for file in files:
        print(file)


if __name__ == "__main__":
    main()
