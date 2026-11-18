"""Execute the retained 100k correctness case separately from the developer set."""
import re
import subprocess
import sys
result=subprocess.run([sys.argv[1],'tests::inventory_projection_is_chunked_and_core_growth_does_not_follow_file_count','--exact','--nocapture'],capture_output=True,text=True,timeout=120)
print(result.stdout+result.stderr)
raise SystemExit(0 if result.returncode==0 and re.search(r'test result: ok\. 1 passed; 0 failed; 0 ignored;',result.stdout) else 1)
