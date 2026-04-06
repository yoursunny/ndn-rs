window.BENCHMARK_DATA = {
  "lastUpdate": 1775515063817,
  "repoUrl": "https://github.com/Quarmire/ndn-rs",
  "entries": {
    "NDN-RS Benchmarks": [
      {
        "commit": {
          "author": {
            "email": "28873711+Quarmire@users.noreply.github.com",
            "name": "Quarmire",
            "username": "Quarmire"
          },
          "committer": {
            "email": "28873711+Quarmire@users.noreply.github.com",
            "name": "Quarmire",
            "username": "Quarmire"
          },
          "distinct": true,
          "id": "1a812eb1141c0618d0c6f082575827dc47048ad1",
          "message": "Fix ndn-python doctests: use python fenced blocks instead of rST\n\nThe indented code blocks (rST `::` syntax) were being compiled as\nRust doctests. Replace with ```python fenced blocks which rustdoc\ncorrectly skips.\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>",
          "timestamp": "2026-04-06T17:20:01-05:00",
          "tree_id": "79d449c6225c943bb16dd268b3dfe79949769d0f",
          "url": "https://github.com/Quarmire/ndn-rs/commit/1a812eb1141c0618d0c6f082575827dc47048ad1"
        },
        "date": 1775515063350,
        "tool": "cargo",
        "benches": [
          {
            "name": "decode/interest/4",
            "value": 666,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "decode/interest/8",
            "value": 764,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "decode/data/4",
            "value": 499,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "decode/data/8",
            "value": 601,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "cs/hit",
            "value": 928,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "cs/miss",
            "value": 617,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "pit/new_entry",
            "value": 1467,
            "range": "± 5",
            "unit": "ns/iter"
          },
          {
            "name": "pit/aggregate",
            "value": 2427,
            "range": "± 102",
            "unit": "ns/iter"
          },
          {
            "name": "fib/lpm/10",
            "value": 54,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "fib/lpm/100",
            "value": 149,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "fib/lpm/1000",
            "value": 149,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "pit_match/hit",
            "value": 1850,
            "range": "± 18",
            "unit": "ns/iter"
          },
          {
            "name": "pit_match/miss",
            "value": 1020,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "cs_insert/insert_replace",
            "value": 1063,
            "range": "± 7",
            "unit": "ns/iter"
          },
          {
            "name": "cs_insert/insert_new",
            "value": 65471,
            "range": "± 63314",
            "unit": "ns/iter"
          },
          {
            "name": "validation_stage/disabled",
            "value": 688,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "validation_stage/cert_via_anchor",
            "value": 37942,
            "range": "± 152",
            "unit": "ns/iter"
          },
          {
            "name": "interest_pipeline/no_route/4",
            "value": 1908,
            "range": "± 9",
            "unit": "ns/iter"
          },
          {
            "name": "interest_pipeline/no_route/8",
            "value": 2084,
            "range": "± 10",
            "unit": "ns/iter"
          },
          {
            "name": "interest_pipeline/cs_hit",
            "value": 1180,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "data_pipeline/4",
            "value": 2230,
            "range": "± 77",
            "unit": "ns/iter"
          },
          {
            "name": "data_pipeline/8",
            "value": 2629,
            "range": "± 88",
            "unit": "ns/iter"
          },
          {
            "name": "decode_throughput/4",
            "value": 667332,
            "range": "± 625",
            "unit": "ns/iter"
          },
          {
            "name": "decode_throughput/8",
            "value": 774762,
            "range": "± 1188",
            "unit": "ns/iter"
          },
          {
            "name": "appface/latency/64",
            "value": 524,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "appface/latency/1024",
            "value": 526,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "appface/latency/8192",
            "value": 523,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "appface/throughput/64",
            "value": 180419,
            "range": "± 587",
            "unit": "ns/iter"
          },
          {
            "name": "appface/throughput/1024",
            "value": 180455,
            "range": "± 267",
            "unit": "ns/iter"
          },
          {
            "name": "appface/throughput/8192",
            "value": 180563,
            "range": "± 243",
            "unit": "ns/iter"
          },
          {
            "name": "unix/latency/64",
            "value": 4404,
            "range": "± 25",
            "unit": "ns/iter"
          },
          {
            "name": "unix/latency/1024",
            "value": 4868,
            "range": "± 14",
            "unit": "ns/iter"
          },
          {
            "name": "unix/latency/8192",
            "value": 7155,
            "range": "± 26",
            "unit": "ns/iter"
          },
          {
            "name": "unix/throughput/64",
            "value": 235039,
            "range": "± 704",
            "unit": "ns/iter"
          },
          {
            "name": "unix/throughput/1024",
            "value": 276162,
            "range": "± 725",
            "unit": "ns/iter"
          },
          {
            "name": "unix/throughput/8192",
            "value": 475231,
            "range": "± 2359",
            "unit": "ns/iter"
          },
          {
            "name": "name/parse/components/4",
            "value": 188,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "name/parse/components/8",
            "value": 469,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "name/parse/components/12",
            "value": 653,
            "range": "± 25",
            "unit": "ns/iter"
          },
          {
            "name": "name/tlv_decode/components/4",
            "value": 151,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "name/tlv_decode/components/8",
            "value": 255,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "name/tlv_decode/components/12",
            "value": 373,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "name/hash/components/4",
            "value": 72,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "name/hash/components/8",
            "value": 132,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "name/eq/eq_match",
            "value": 22,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "name/eq/eq_miss_first",
            "value": 2,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "name/eq/eq_miss_last",
            "value": 20,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "name/has_prefix/prefix_len/1",
            "value": 5,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "name/has_prefix/prefix_len/4",
            "value": 14,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "name/has_prefix/prefix_len/8",
            "value": 25,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "name/display/components/4",
            "value": 411,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "name/display/components/8",
            "value": 804,
            "range": "± 6",
            "unit": "ns/iter"
          },
          {
            "name": "signing/ed25519/sign_sync/100B",
            "value": 18534,
            "range": "± 73",
            "unit": "ns/iter"
          },
          {
            "name": "signing/ed25519/sign_sync/500B",
            "value": 20191,
            "range": "± 26",
            "unit": "ns/iter"
          },
          {
            "name": "signing/hmac/sign_sync/100B",
            "value": 260,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "signing/hmac/sign_sync/500B",
            "value": 549,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "verification/ed25519/verify/100B",
            "value": 37476,
            "range": "± 69",
            "unit": "ns/iter"
          },
          {
            "name": "verification/ed25519/verify/500B",
            "value": 38965,
            "range": "± 84",
            "unit": "ns/iter"
          },
          {
            "name": "validation/schema_mismatch",
            "value": 200,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "validation/cert_missing",
            "value": 252,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "validation/single_hop",
            "value": 37070,
            "range": "± 54",
            "unit": "ns/iter"
          },
          {
            "name": "lru/get_miss_empty",
            "value": 191,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "lru/get_miss_populated",
            "value": 231,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "lru/get_hit",
            "value": 265,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "lru/get_can_be_prefix",
            "value": 389,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "lru/insert_replace",
            "value": 399,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "lru/insert_new",
            "value": 2389,
            "range": "± 1392",
            "unit": "ns/iter"
          },
          {
            "name": "lru/evict",
            "value": 246,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "lru/evict_prefix",
            "value": 3935,
            "range": "± 2905",
            "unit": "ns/iter"
          },
          {
            "name": "sharded/get_hit/1",
            "value": 288,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sharded/insert/1",
            "value": 2907,
            "range": "± 998",
            "unit": "ns/iter"
          },
          {
            "name": "sharded/get_hit/4",
            "value": 296,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sharded/insert/4",
            "value": 3222,
            "range": "± 1212",
            "unit": "ns/iter"
          },
          {
            "name": "sharded/get_hit/8",
            "value": 288,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "sharded/insert/8",
            "value": 3362,
            "range": "± 1644",
            "unit": "ns/iter"
          },
          {
            "name": "sharded/get_hit/16",
            "value": 293,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sharded/insert/16",
            "value": 3317,
            "range": "± 1502",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}