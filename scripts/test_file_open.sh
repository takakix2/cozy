#!/bin/bash
# ファイルを引数で開く機能のテスト

set -e

echo "=== ファイルを引数で開く機能のテスト ==="
echo ""

# テストファイルを作成
TEST_FILE="test_open.rs"
echo 'fn main() {
    let x = 42;
    println!("Hello, world! {}", x);
}' > "$TEST_FILE"

echo "1. テストファイルを作成: $TEST_FILE"
echo "   内容:"
cat "$TEST_FILE" | sed 's/^/   /'
echo ""

# ビルド確認
echo "2. ビルド確認..."
if cargo build --release > /dev/null 2>&1; then
    echo "   ✓ ビルド成功"
else
    echo "   ✗ ビルド失敗"
    exit 1
fi

# 引数なしで起動（新規ファイル）
echo ""
echo "3. 引数なしで起動（新規ファイルモード）:"
echo "   コマンド: cargo run --release"
echo "   期待動作: 空のエディタが起動"
echo "   （実際に起動して確認してください）"
echo ""

# 引数ありで起動（既存ファイル）
echo "4. 引数ありで起動（既存ファイルを開く）:"
echo "   コマンド: cargo run --release $TEST_FILE"
echo "   期待動作: $TEST_FILE の内容が読み込まれてエディタが起動"
echo "   （実際に起動して確認してください）"
echo ""

# 存在しないファイル
echo "5. 存在しないファイルを指定:"
echo "   コマンド: cargo run --release nonexistent.rs"
echo "   期待動作: 新規ファイルとして起動（ファイルが作成される）"
echo "   （実際に起動して確認してください）"
echo ""

# 複数ファイル（対応していない）
echo "6. 複数ファイルを指定（未対応）:"
echo "   コマンド: cargo run --release file1.rs file2.rs"
echo "   期待動作: 最初のファイルのみ開く（警告なし）"
echo "   （実際に起動して確認してください）"
echo ""

echo "=== テストファイル ==="
echo "テストファイル: $TEST_FILE"
echo ""
echo "実際にテストするには:"
echo "  cargo run --release $TEST_FILE"
echo ""
echo "終了後、テストファイルを削除:"
echo "  rm $TEST_FILE"
