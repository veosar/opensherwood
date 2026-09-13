// Decompiles every function of the program into <out dir>/decomp_all/<address>.c with a callers / callees
// header, so analysts can read and grep the whole program without holding the Ghidra project open (ADR-0009
// analyst tooling; the output stays in the ignored re/). Argument: the output directory (default re/out).
//@category OpenSherwood
import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import java.io.*;
import java.util.*;

public class DecompileAll extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        File dir = new File(new File(args.length > 0 ? args[0] : "re/out"), "decomp_all");
        dir.mkdirs();
        DecompInterface di = new DecompInterface();
        di.toggleCCode(true);
        di.setSimplificationStyle("decompile");
        di.openProgram(currentProgram);
        int n = 0;
        FunctionIterator it = currentProgram.getFunctionManager().getFunctions(true);
        while (it.hasNext() && !monitor.isCancelled()) {
            Function f = it.next();
            File out = new File(dir, f.getEntryPoint() + ".c");
            if (out.exists()) { n++; continue; }
            DecompileResults res = di.decompileFunction(f, 90, monitor);
            try (PrintWriter w = new PrintWriter(new FileWriter(out))) {
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
            n++;
            if (n % 500 == 0) println("decompiled " + n);
        }
        di.dispose();
        println("decompiled " + n + " functions into " + dir);
    }
}
