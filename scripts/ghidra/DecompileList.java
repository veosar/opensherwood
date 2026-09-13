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
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 1) { println("usage: DecompileList <list file> [out dir]"); return; }
        File outDir = new File(args.length > 1 ? args[1] : "re/out");
        File dir = new File(outDir, "decomp");
        dir.mkdirs();
        DecompInterface di = new DecompInterface();
        di.toggleCCode(true);
        di.setSimplificationStyle("decompile");
        di.openProgram(currentProgram);
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
                    w.println("// decompilation failed: " + res.getErrorMessage());
                }
            }
        }
        di.dispose();
        println("decompilation written to " + dir);
    }
}
