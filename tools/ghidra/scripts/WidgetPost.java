// Post-analysis for the Borland C++ binaries: many methods are only reached
// through vtables, which Ghidra's analysis doesn't follow. Every
// `push ebp; mov ebp,esp` (55 8b ec) in an executable block that isn't inside
// a function yet becomes one, and so does orphan code Ghidra disassembled but
// never put into a function. Repeat until nothing changes.
//@category WidgetWorkshop

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;
import ghidra.program.model.mem.MemoryBlock;

public class WidgetPost extends GhidraScript {
	@Override
	protected void run() throws Exception {
		int created = 0;
		for (int pass = 0; pass < 6; pass++) {
			int n = prologues() + orphans();
			created += n;
			if (n == 0) {
				break;
			}
		}
		println("WidgetPost: created " + created + " functions");
		// Borland's register-passing helpers (exception frames and the like)
		// make Ghidra guess __register for their callers; real parameters show
		// up once the decompiler-based parameter analysis has run.
		for (Function f : currentProgram.getFunctionManager().getFunctions(true)) {
			if (!f.isThunk() && "__register".equals(f.getCallingConventionName())) {
				f.setCallingConvention("__cdecl");
			}
		}
		setAnalysisOption(currentProgram, "Decompiler Parameter ID", "true");
		analyzeAll(currentProgram);
		println("WidgetPost: parameter analysis done");
	}

	private int prologues() throws Exception {
		FunctionManager fm = currentProgram.getFunctionManager();
		Listing listing = currentProgram.getListing();
		int created = 0;
		for (MemoryBlock b : currentProgram.getMemory().getBlocks()) {
			if (!b.isExecute() || !b.isInitialized()) {
				continue;
			}
			byte[] bytes = new byte[(int) b.getSize()];
			b.getBytes(b.getStart(), bytes);
			for (int i = 0; i + 3 <= bytes.length; i++) {
				if ((bytes[i] & 0xff) != 0x55 || (bytes[i + 1] & 0xff) != 0x8b || (bytes[i + 2] & 0xff) != 0xec) {
					continue;
				}
				Address a = b.getStart().add(i);
				if (fm.getFunctionContaining(a) != null) {
					continue;
				}
				Data d = listing.getDefinedDataContaining(a);
				if (d != null) {
					continue;
				}
				if (listing.getInstructionAt(a) == null) {
					disassemble(a);
				}
				if (createFunction(a, null) != null) {
					created++;
				}
			}
		}
		return created;
	}

	private int orphans() throws Exception {
		Listing listing = currentProgram.getListing();
		FunctionManager fm = currentProgram.getFunctionManager();
		int created = 0;
		for (MemoryBlock b : currentProgram.getMemory().getBlocks()) {
			if (!b.isExecute()) {
				continue;
			}
			InstructionIterator it = listing.getInstructions(b.getStart(), true);
			while (it.hasNext() && !monitor.isCancelled()) {
				Instruction insn = it.next();
				Address a = insn.getAddress();
				if (!b.contains(a)) {
					break;
				}
				if (fm.getFunctionContaining(a) != null) {
					continue;
				}
				Instruction prev = listing.getInstructionBefore(a);
				if (prev != null && a.equals(prev.getFallThrough())) {
					continue;
				}
				Function f = createFunction(a, null);
				if (f != null) {
					created++;
					it = listing.getInstructions(f.getBody().getMaxAddress().next(), true);
				}
			}
		}
		return created;
	}
}
