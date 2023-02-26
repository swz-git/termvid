cargo build --release --target x86_64-pc-windows-gnu
WINEDEBUG=-all wine ./target/x86_64-pc-windows-gnu/release/termvid.exe $1 $2 $3 $4 $5 $6 $7 $8 $9