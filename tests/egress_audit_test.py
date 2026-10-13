"""Observe destination metadata in an isolated exact-RPM workflow.

No installation, personal database, system tracing configuration or external
service is needed. Dynamic LD_PRELOAD interception is deliberately narrower than
an OS audit. --self-test uses only temporary loopback sockets/numeric resolution.
The release mode reuses extracted_release_test without modifying its source.
"""
import argparse
import hashlib
import importlib.util
import ipaddress
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
HOOK = ROOT / 'tests/egress_audit_hook.c'
LIMITATIONS = [
    'Dynamic-symbol observation only; direct syscalls, static binaries, hidden/internal libc calls and noninterposed send/sendmsg/write paths can bypass it.',
    'Descendants that drop LD_PRELOAD, sandboxed/helper processes that reject it, and activity outside the owned fixture process tree are not covered.',
    'getaddrinfo records resolver inputs, not proof of actual DNS packets; resolver caches, hosts files and internal resolver paths are not independently traced.',
    'Attempts are recorded before the call; neither success nor packet delivery is established.',
    'Instrumentation can affect execution and timing. This is neither a firewall nor a complete OS network audit.',
]


def compile_hook(directory):
    library = directory / 'egress-audit.so'
    subprocess.run(['cc', '-shared', '-fPIC', '-Wall', '-Wextra', '-Werror',
                    str(HOOK), '-o', str(library), '-ldl', '-pthread'], check=True, capture_output=True)
    return library


def read_trace(path):
    events = [json.loads(line) for line in path.read_text().splitlines()]
    fields = {'event', 'pid', 'family', 'protocol', 'address', 'port', 'hostname'}
    for event in events:
        assert set(event) == fields, 'Unexpected audit metadata fields'
        assert event['event'] in {'loaded', 'socket', 'connect', 'sendto', 'getaddrinfo'}
        assert isinstance(event['pid'], int) and event['pid'] > 0
        assert isinstance(event['port'], int) and 0 <= event['port'] <= 65535
        assert isinstance(event['family'], int)
        assert event['protocol'] in {'none', 'tcp', 'udp', 'other'}
        assert isinstance(event['address'], str) and isinstance(event['hostname'], str)
    assert any(event['event'] == 'loaded' for event in events), 'Audit preload did not record startup'
    return events


def is_loopback(address):
    try:
        value = ipaddress.ip_address(address)
        return value.is_loopback or (isinstance(value, ipaddress.IPv6Address)
                                    and value.ipv4_mapped is not None and value.ipv4_mapped.is_loopback)
    except ValueError:
        return False


def summarize(events):
    destinations = [event for event in events if event['event'] in {'connect', 'sendto'}]
    resolver_inputs = [event for event in events if event['event'] == 'getaddrinfo' and event['hostname']]
    forbidden_dns = []
    numeric_outside_loopback = []
    for event in resolver_inputs:
        host = event['hostname']
        try:
            ipaddress.ip_address(host)
            if not is_loopback(host):
                numeric_outside_loopback.append(event)
        except ValueError:
            if host.rstrip('.').lower() != 'localhost':
                forbidden_dns.append(event)
    return {
        'observedProcessIds': sorted({event['pid'] for event in events}),
        'preloadStartupProcessIds': sorted({event['pid'] for event in events if event['event'] == 'loaded'}),
        'eventCounts': {kind: sum(event['event'] == kind for event in events)
                        for kind in ['loaded', 'socket', 'connect', 'sendto', 'getaddrinfo']},
        'loopbackAttempts': [event for event in destinations if is_loopback(event['address'])],
        'nonLoopbackAttempts': [event for event in destinations if not is_loopback(event['address'])],
        'forbiddenHostnameResolverInputs': forbidden_dns,
        'numericResolverInputsOutsideLoopback': numeric_outside_loopback,
    }


DRIVER = r'''
#include <arpa/inet.h>
#include <netdb.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <sys/wait.h>
#include <unistd.h>
#include <assert.h>
int main(int argc, char **argv) {
    const char *canary = "synthetic-payload-credential-canary";
    struct addrinfo hints = {0}, *addresses = NULL;
    hints.ai_flags = AI_NUMERICHOST | AI_NUMERICSERV;
    hints.ai_socktype = SOCK_STREAM;
    assert(getaddrinfo("127.0.0.1", "43210", &hints, &addresses) == 0);
    freeaddrinfo(addresses); addresses = NULL;
    assert(getaddrinfo("::1", "43210", &hints, &addresses) == 0);
    freeaddrinfo(addresses);
    /* A service string is never included in the log, even on failure. */
    assert(getaddrinfo("127.0.0.1", canary, &hints, &addresses) != 0);
    if (argc > 1) return 0;
    int server = socket(AF_INET, SOCK_STREAM, 0);
    assert(server >= 0);
    struct sockaddr_in address = {0}; address.sin_family = AF_INET;
    address.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    assert(bind(server, (struct sockaddr *)&address, sizeof address) == 0);
    assert(listen(server, 1) == 0);
    socklen_t size = sizeof address;
    assert(getsockname(server, (struct sockaddr *)&address, &size) == 0);
    int client = socket(AF_INET, SOCK_STREAM, 0);
    assert(connect(client, (struct sockaddr *)&address, sizeof address) == 0);
    int accepted = accept(server, NULL, NULL); assert(accepted >= 0);
    close(client); close(accepted); close(server);
    int udp = socket(AF_INET, SOCK_DGRAM, 0); assert(udp >= 0);
    address.sin_port = 0;
    assert(bind(udp, (struct sockaddr *)&address, sizeof address) == 0);
    size = sizeof address; assert(getsockname(udp, (struct sockaddr *)&address, &size) == 0);
    assert(sendto(udp, canary, strlen(canary), 0, (struct sockaddr *)&address, sizeof address) == (ssize_t)strlen(canary));
    char buffer[128]; assert(recv(udp, buffer, sizeof buffer, 0) == (ssize_t)strlen(canary));close(udp);
    int unix_fd = socket(AF_UNIX, SOCK_STREAM, 0);
    struct sockaddr_un local = {0}; local.sun_family = AF_UNIX;
    snprintf(local.sun_path, sizeof local.sun_path, "/tmp/%s-not-present", canary);
    assert(connect(unix_fd, (struct sockaddr *)&local, sizeof local) == -1);close(unix_fd);
    pid_t child = fork(); assert(child >= 0);
    if (!child) {execl(argv[0], argv[0], "child", (char *)NULL); _exit(99);}
    int status; assert(waitpid(child, &status, 0) == child);
    assert(WIFEXITED(status) && WEXITSTATUS(status) == 0);
    return 0;
}
'''


def self_test(report_path=None):
    with tempfile.TemporaryDirectory(prefix='novel-egress-self-test-') as directory:
        folder = Path(directory)
        library = compile_hook(folder)
        driver_source = folder / 'driver.c'
        driver_source.write_text(DRIVER)
        driver = folder / 'driver'
        subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', str(driver_source), '-o', str(driver)],
                       check=True, capture_output=True)
        trace = folder / 'audit.jsonl'
        environment = {key: value for key, value in os.environ.items()
                       if key not in {'LD_PRELOAD', 'EGRESS_AUDIT_LOG'}}
        environment.update(LD_PRELOAD=str(library), EGRESS_AUDIT_LOG=str(trace))
        child = subprocess.run([str(driver)], env=environment, check=True, timeout=10, capture_output=True)
        assert child.stdout == b'' and child.stderr == b''
        events = read_trace(trace)
        result = summarize(events)
        assert len(result['preloadStartupProcessIds']) == 2, 'Exec child did not inherit audit preload'
        assert result['eventCounts']['socket'] == 3
        assert result['eventCounts']['connect'] == 1 and result['eventCounts']['sendto'] == 1
        assert result['eventCounts']['getaddrinfo'] == 6
        assert len(result['loopbackAttempts']) == 2
        assert not result['nonLoopbackAttempts'] and not result['forbiddenHostnameResolverInputs']
        assert not result['numericResolverInputsOutsideLoopback']
        assert 'synthetic-payload-credential-canary' not in trace.read_text()
        assert '/tmp/' not in trace.read_text(), 'Unix socket path leaked'
        # Negative policy checks use metadata only: no external resolver/socket.
        sample = {'event': 'connect', 'pid': 1, 'family': 2, 'protocol': 'tcp',
                  'address': '203.0.113.1', 'port': 443, 'hostname': ''}
        assert len(summarize([sample])['nonLoopbackAttempts']) == 1
        sample.update(event='getaddrinfo', address='', hostname='example.invalid')
        assert len(summarize([sample])['forbiddenHostnameResolverInputs']) == 1
        sample.update(hostname='100.64.1.2')
        assert len(summarize([sample])['numericResolverInputsOutsideLoopback']) == 1
        assert is_loopback('::ffff:127.0.0.1') and not is_loopback('100.64.1.2')
        result.update(mode='synthetic-self-test', passed=True,
                      checks=['IPv4 TCP connect and UDP sendto destinations observed',
                              'Numeric IPv4/IPv6 resolver inputs observed without external DNS',
                              'Exec child inherited instrumentation',
                              'Payload, service string and Unix socket path canaries absent',
                              'Non-loopback address and forbidden hostname classification rejects synthetic metadata'],
                      limitations=LIMITATIONS)
    if report_path:
        report_path.parent.mkdir(parents=True, exist_ok=True)
        report_path.write_text(json.dumps(result, indent=2) + '\n')
    return result


def run_release(rpm, report_path, offline=True, scenes=1):
    """Compose interposers for exactly the existing production workflow fixture.

    Only the hook compile command and owned app Popen are wrapped. This avoids
    altering production code or the shared release fixture. Restoration is in
    finally even if the workflow fails. Compile/extraction/seeding run normally;
    the audit starts at application launch, then follows inheriting descendants.
    """
    spec = importlib.util.spec_from_file_location('novel_owned_release_fixture', ROOT / 'tests/extracted_release_test.py')
    fixture = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(fixture)
    original_run, original_popen = subprocess.run, subprocess.Popen
    original_log = os.environ.get('EGRESS_AUDIT_LOG')
    with tempfile.TemporaryDirectory(prefix='novel-egress-release-') as directory:
        folder = Path(directory)
        trace = folder / 'audit.jsonl'
        workflow_report = folder / 'workflow.json'
        trace_destination = report_path.with_suffix('.destinations.jsonl')
        app_pids = []
        composed = []

        def instrument_compile(command, *args, **kwargs):
            if isinstance(command, list) and command and command[0] == 'cc' and str(ROOT / 'tests/release_webkit_hook.c') in command:
                command = [*command, str(HOOK), '-pthread']
                composed.append(True)
            return original_run(command, *args, **kwargs)

        def identify_app(command, *args, **kwargs):
            process = original_popen(command, *args, **kwargs)
            if isinstance(command, list) and any(str(item).endswith('/usr/bin/novel-workspace') for item in command):
                app_pids.append(process.pid)
            return process

        result = {'mode': 'exact-extracted-rpm', 'passed': False,
                  'rpmSha256': hashlib.sha256(rpm.read_bytes()).hexdigest(),
                  'limitations': LIMITATIONS, 'networkNamespaceDisabled': offline,
                  'auditScope': 'Application launch and descendants inheriting the composite preload; extraction/seed process is outside audit scope.'}
        try:
            os.environ['EGRESS_AUDIT_LOG'] = str(trace)
            subprocess.run, subprocess.Popen = instrument_compile, identify_app
            fixture.run(rpm, workflow_report, offline=offline, installed=False, scenes=scenes)
            assert composed == [True], 'Existing WebKit hook was not composed with the network audit'
            events = read_trace(trace)
            observed = summarize(events)
            # unshare execs the application in-place, retaining the Popen PID.
            assert app_pids and all(sum(event['event'] == 'loaded' and event['pid'] == pid for event in events) >= (2 if offline else 1) for pid in app_pids), 'The launched application did not reload the audit preload after its namespace launcher'
            result.update(observed, workflow=json.loads(workflow_report.read_text()))
            assert not observed['nonLoopbackAttempts'], 'Observed non-loopback network attempt'
            assert not observed['forbiddenHostnameResolverInputs'], 'Observed forbidden hostname resolver input'
            assert not observed['numericResolverInputsOutsideLoopback'], 'Observed non-loopback numeric resolver input'
            result['passed'] = True
        except Exception as error:
            result['error'] = str(error)
            if trace.exists():
                try: result.update(summarize(read_trace(trace)))
                except Exception as trace_error: result['traceError'] = str(trace_error)
            raise
        finally:
            subprocess.run, subprocess.Popen = original_run, original_popen
            if original_log is None: os.environ.pop('EGRESS_AUDIT_LOG', None)
            else: os.environ['EGRESS_AUDIT_LOG'] = original_log
            report_path.parent.mkdir(parents=True, exist_ok=True)
            if trace.exists():
                trace_destination.write_bytes(trace.read_bytes())
                result['destinationTrace'] = str(trace_destination.resolve())
            report_path.write_text(json.dumps(result, indent=2) + '\n')
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--self-test', action='store_true')
    parser.add_argument('--rpm', type=Path)
    parser.add_argument('--report', type=Path, default=ROOT / 'work/egress-audit.json')
    parser.add_argument('--scenes', type=int, choices=[1, 1000], default=1)
    parser.add_argument('--network-enabled', action='store_true', help='Observe the owned fixture without the network namespace; does not configure app services.')
    args = parser.parse_args()
    if args.self_test:
        result = self_test(args.report)
    else:
        parser.error('--rpm is required outside --self-test') if not args.rpm else None
        self_test()
        result = run_release(args.rpm.resolve(), args.report.resolve(), offline=not args.network_enabled, scenes=args.scenes)
    print(json.dumps(result, indent=2))
