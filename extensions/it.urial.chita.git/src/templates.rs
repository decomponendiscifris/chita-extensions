//! `.gitignore` templates the editor offers to insert: short, for what people keep in
//! repositories on a Mac.

pub const TEMPLATES: &[(&str, &str)] = &[
    (
        "macOS",
        "# macOS\n.DS_Store\n.AppleDouble\n.LSOverride\n._*\n.Spotlight-V100\n.Trashes\n",
    ),
    (
        "Xcode",
        "# Xcode\nbuild/\nDerivedData/\n*.xcuserstate\nxcuserdata/\n*.moved-aside\n*.hmap\n*.ipa\n*.dSYM.zip\n*.dSYM\n",
    ),
    (
        "Swift package",
        "# Swift Package Manager\n.build/\n.swiftpm/\nPackage.resolved\n*.xcodeproj\n",
    ),
    ("Rust", "# Rust\n/target/\n**/*.rs.bk\n"),
    (
        "Node",
        "# Node\nnode_modules/\nnpm-debug.log*\nyarn-error.log*\n.pnpm-store/\ndist/\n.env\n.env.local\n",
    ),
    (
        "Python",
        "# Python\n__pycache__/\n*.py[cod]\n.venv/\nvenv/\n*.egg-info/\n.pytest_cache/\n.mypy_cache/\ndist/\nbuild/\n",
    ),
    ("Go", "# Go\n/bin/\n*.exe\n*.test\n*.out\nvendor/\n"),
    (
        "Java and Kotlin",
        "# Java and Kotlin\n*.class\n*.jar\n*.war\ntarget/\nbuild/\n.gradle/\nout/\n*.iml\n.idea/\n",
    ),
    ("Editors", "# Editors\n.vscode/\n.idea/\n*.swp\n*~\n"),
];
