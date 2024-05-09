while :
do
    cargo test --bin magpie -- --nocapture
    ret_code=$?
    if [ $ret_code != 0 ]; then
      exit 1
    fi
done
