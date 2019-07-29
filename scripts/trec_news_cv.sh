#!/bin/bash

#DIR="${HOME}/code/irene/scripts"
DIR="news-bg-v4"

cargo build --release
for x in 0 1 2 3 4; do
  ./target/release/coordinate_ascent ${DIR}/train${x}.ranklib \
    --test ${DIR}/test${x}.ranklib \
    --feature_names ${DIR}/feature_names.json \
    --normalize_weights \
    --metric2t ndcg \
    --seed 42 | tee logs/v4_1.${x}
  
  ./target/release/train_random_forest ${DIR}/train${x}.ranklib \
  --test ${DIR}/test${x}.ranklib \
  --feature_names ${DIR}/feature_names.json \
  --metric2t ndcg \
  --srate 0.5 \
  --frate 0.5 \
  --split_candidates 16 \
  --max_depth 7 \
  --seed 42 | tee logs/v4_1.rf.${x}
done

