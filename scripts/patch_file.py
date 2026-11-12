import sys

formula_file = sys.argv[1]
with open(formula_file, "r", encoding="utf-8") as f:
    content = f.read()

if "service do" not in content:
    service_block = '''
  service do
    run [opt_bin/"pcbfapi", "run"]
    keep_alive true
    run_type :immediate
    environment_variables PATH: std_service_path_env
    error_log_path var/"log/pcbfapi.err.log"
    log_path var/"log/pcbfapi.log"
    working_dir var
  end

  def post_install
    (var/"log").mkpath
    system "#{opt_bin}/pcbfapi", "service", "install" rescue nil
  end

  def caveats
    <<~EOS
      pcbfapi runs as a persistent background daemon service.
      It starts automatically at boot/login.

      To manage with brew services:
        brew services start pcbfapi
        brew services stop pcbfapi
        brew services restart pcbfapi

      To verify KiCad file and CLI access:
        pcbfapi check-access
    EOS
  end
'''
    idx = content.rfind("end")
    if idx != -1:
        content = content[:idx] + service_block + content[idx:]
        with open(formula_file, "w", encoding="utf-8") as f:
            f.write(content)