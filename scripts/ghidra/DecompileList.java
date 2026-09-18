// Decompiles the functions listed (one address per line, hex) in the file given as the first argument and
// writes each to <out dir>/decomp/<address>.c, plus a callers/callees summary line (ADR-0009 analyst tooling;
// the output stays in the ignored re/). Second argument: the output directory (default re/out).
//@category OpenSherwood
import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;
import java.io.*;
import java.nio.file.*;
import java.util.*;

public class DecompileList extends GhidraScript {

    /** The output directory must be inside the analysis root (a directory named `re`); anything else is refused. */
    static File analysisOut(String[] args, int index) throws IOException {
        File out = new File(args.length > index ? args[index] : "re/out").getCanonicalFile();
        for (File p = out; p != null; p = p.getParentFile()) {
            if (p.getName().equals("re")) { out.mkdirs(); return out; }
        }
        throw new IOException("refusing to write outside the analysis root (a directory named re): " + out);
    }

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 1) { println("usage: DecompileList <list file> [out dir]"); return; }
        File outDir = analysisOut(args, 1);
        File dir = new File(outDir, args.length > 2 ? "decomp_" + args[2] : "decomp");
        dir.mkdirs();
        DecompInterface di = new DecompInterface();
        di.toggleCCode(true);
        // Optional third argument: the simplification style ("decompile", "normalize", "firstpass", "register", "paramid").
        di.setSimplificationStyle(args.length > 2 ? args[2] : "decompile");
        if (!di.openProgram(currentProgram)) throw new IOException("decompiler failed to open the program: " + di.getLastMessage());
        int failed = 0;
        try {
        FunctionManager fm = currentProgram.getFunctionManager();
        for (String line : Files.readAllLines(Paths.get(args[0]))) {
            String s = line.trim();
            if (s.isEmpty() || s.startsWith("#")) continue;
            if (s.startsWith("0x")) s = s.substring(2);
            Address a = currentProgram.getAddressFactory().getDefaultAddressSpace().getAddress(Long.parseLong(s, 16));
            Function f = fm.getFunctionContaining(a);
            if (f == null) { println("no function at " + s); continue; }
            DecompileResults res = di.decompileFunction(f, 120, monitor);
            try (PrintWriter w = new PrintWriter(new FileWriter(new File(dir, f.getEntryPoint() + ".c")))) {
                w.println("// " + f.getName() + " at " + f.getEntryPoint() + " size " + f.getBody().getNumAddresses());
                Set<String> callers = new TreeSet<>();
                for (Function c : f.getCallingFunctions(monitor)) callers.add(c.getEntryPoint().toString());
                Set<String> callees = new TreeSet<>();
                for (Function c : f.getCalledFunctions(monitor)) callees.add(c.getEntryPoint().toString());
                w.println("// callers: " + String.join(",", callers));
                w.println("// callees: " + String.join(",", callees));
                if (res.decompileCompleted() && res.getDecompiledFunction() != null) {
                    w.println(res.getDecompiledFunction().getC());
                } else {
                    failed++;
                    w.println("// decompilation failed: " + res.getErrorMessage());
                }
            }
        }
        } finally {
            di.dispose();
        }
        if (monitor.isCancelled()) throw new IOException("cancelled: the output is incomplete");
        println("decompilation failures: " + failed);
        println("decompilation written to " + dir);
    }
}
