ffffffff80102420 <kmain>:
ffffffff80102420: 55                   	pushq	%rbp
ffffffff80102421: 48 89 e5             	movq	%rsp, %rbp
ffffffff80102424: 41 57                	pushq	%r15
ffffffff80102426: 41 56                	pushq	%r14
ffffffff80102428: 41 55                	pushq	%r13
ffffffff8010242a: 41 54                	pushq	%r12
ffffffff8010242c: 53                   	pushq	%rbx
ffffffff8010242d: 48 83 ec 68          	subq	$0x68, %rsp
ffffffff80102431: 48 89 f3             	movq	%rsi, %rbx
ffffffff80102434: e8 17 82 00 00       	callq	0xffffffff8010a650 <kernel__drivers__serial__init>
ffffffff80102439: 48 8d 3d 60 71 02 00 	leaq	0x27160(%rip), %rdi     # 0xffffffff801295a0 <str_145>
ffffffff80102440: e8 4b 83 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff80102445: 48 8d 3d 74 71 02 00 	leaq	0x27174(%rip), %rdi     # 0xffffffff801295c0 <str_152>
ffffffff8010244c: e8 3f 83 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff80102451: e8 ca 76 01 00       	callq	0xffffffff80119b20 <gdt_init>
ffffffff80102456: 48 8d 3d 83 71 02 00 	leaq	0x27183(%rip), %rdi     # 0xffffffff801295e0 <str_159>
ffffffff8010245d: e8 2e 83 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff80102462: e8 89 77 01 00       	callq	0xffffffff80119bf0 <kernel__arch__x86__idt__init>
ffffffff80102467: 48 8d 3d 92 71 02 00 	leaq	0x27192(%rip), %rdi     # 0xffffffff80129600 <str_166>
ffffffff8010246e: e8 1d 83 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff80102473: e8 18 80 00 00       	callq	0xffffffff8010a490 <pit_init>
ffffffff80102478: 48 8d 3d a1 71 02 00 	leaq	0x271a1(%rip), %rdi     # 0xffffffff80129620 <str_173>
ffffffff8010247f: e8 0c 83 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff80102484: 48 89 df             	movq	%rbx, %rdi
ffffffff80102487: e8 34 87 01 00       	callq	0xffffffff8011abc0 <multiboot_parse>
ffffffff8010248c: 31 ff                	xorl	%edi, %edi
ffffffff8010248e: e8 6d 81 00 00       	callq	0xffffffff8010a600 <fb_clear>
ffffffff80102493: 48 8d 3d b6 71 02 00 	leaq	0x271b6(%rip), %rdi     # 0xffffffff80129650 <str_184>
ffffffff8010249a: e8 f1 82 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff8010249f: e8 9c 7d 01 00       	callq	0xffffffff8011a240 <run_smp_tests>
ffffffff801024a4: 49 89 c6             	movq	%rax, %r14
ffffffff801024a7: 48 8d 3d c2 71 02 00 	leaq	0x271c2(%rip), %rdi     # 0xffffffff80129670 <str_192>
ffffffff801024ae: e8 dd 82 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff801024b3: 4c 89 f7             	movq	%r14, %rdi
ffffffff801024b6: e8 55 85 00 00       	callq	0xffffffff8010aa10 <kernel__drivers__serial__print_u64>
ffffffff801024bb: 48 8d 1d c8 71 02 00 	leaq	0x271c8(%rip), %rbx     # 0xffffffff8012968a <str_199>
ffffffff801024c2: 48 89 df             	movq	%rbx, %rdi
ffffffff801024c5: e8 c6 82 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff801024ca: 31 ff                	xorl	%edi, %edi
ffffffff801024cc: e8 4f 84 01 00       	callq	0xffffffff8011a920 <acpi_get_apic_id>
ffffffff801024d1: 0f b6 f8             	movzbl	%al, %edi
ffffffff801024d4: e8 27 7f 00 00       	callq	0xffffffff8010a400 <percpu_init_bsp>
ffffffff801024d9: 4c 89 f7             	movq	%r14, %rdi
ffffffff801024dc: e8 8f 28 00 00       	callq	0xffffffff80104d70 <pmm_init_cpu_count>
ffffffff801024e1: 4c 89 f7             	movq	%r14, %rdi
ffffffff801024e4: e8 37 c5 00 00       	callq	0xffffffff8010ea20 <init_cores>
ffffffff801024e9: 48 8d 3d a0 71 02 00 	leaq	0x271a0(%rip), %rdi     # 0xffffffff80129690 <str_210>
ffffffff801024f0: e8 9b 82 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff801024f5: e8 d6 4d 00 00       	callq	0xffffffff801072d0 <sched_init>
ffffffff801024fa: 48 8d 3d af 71 02 00 	leaq	0x271af(%rip), %rdi     # 0xffffffff801296b0 <str_217>
ffffffff80102501: e8 8a 82 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff80102506: e8 d5 69 01 00       	callq	0xffffffff80118ee0 <ecs_world_init>
ffffffff8010250b: 48 89 05 46 ac 04 00 	movq	%rax, 0x4ac46(%rip)     # 0xffffffff8014d158 <kernel__core__main__KERNEL_ECS_WORLD>
ffffffff80102512: 48 8d 3d b7 71 02 00 	leaq	0x271b7(%rip), %rdi     # 0xffffffff801296d0 <str_226>
ffffffff80102519: e8 72 82 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff8010251e: e8 7d ca 00 00       	callq	0xffffffff8010efa0 <slab_cache_init>
ffffffff80102523: 48 8d 3d c6 71 02 00 	leaq	0x271c6(%rip), %rdi     # 0xffffffff801296f0 <str_233>
ffffffff8010252a: e8 61 82 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff8010252f: e8 cc f9 ff ff       	callq	0xffffffff80101f00 <vma_init>
ffffffff80102534: 48 8d 3d d5 71 02 00 	leaq	0x271d5(%rip), %rdi     # 0xffffffff80129710 <str_240>
ffffffff8010253b: e8 50 82 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff80102540: e8 eb 27 00 00       	callq	0xffffffff80104d30 <pmm_alloc>
ffffffff80102545: 49 89 c7             	movq	%rax, %r15
ffffffff80102548: e8 e3 27 00 00       	callq	0xffffffff80104d30 <pmm_alloc>
ffffffff8010254d: 49 89 c6             	movq	%rax, %r14
ffffffff80102550: 48 8d 3d d9 71 02 00 	leaq	0x271d9(%rip), %rdi     # 0xffffffff80129730 <str_249>
ffffffff80102557: e8 34 82 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff8010255c: 4c 89 ff             	movq	%r15, %rdi
ffffffff8010255f: e8 ac 84 00 00       	callq	0xffffffff8010aa10 <kernel__drivers__serial__print_u64>
ffffffff80102564: 48 8d 3d df 71 02 00 	leaq	0x271df(%rip), %rdi     # 0xffffffff8012974a <str_256>
ffffffff8010256b: e8 20 82 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff80102570: 4c 89 f7             	movq	%r14, %rdi
ffffffff80102573: e8 98 84 00 00       	callq	0xffffffff8010aa10 <kernel__drivers__serial__print_u64>
ffffffff80102578: 48 89 df             	movq	%rbx, %rdi
ffffffff8010257b: e8 10 82 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff80102580: 49 81 c7 00 10 00 80 	addq	$-0x7ffff000, %r15      # imm = 0x80001000
ffffffff80102587: 49 81 c6 00 10 00 80 	addq	$-0x7ffff000, %r14      # imm = 0x80001000
ffffffff8010258e: 31 ff                	xorl	%edi, %edi
ffffffff80102590: 4c 89 fe             	movq	%r15, %rsi
ffffffff80102593: e8 a8 7e 00 00       	callq	0xffffffff8010a440 <percpu_set_nmi_stack>
ffffffff80102598: 31 ff                	xorl	%edi, %edi
ffffffff8010259a: 4c 89 f6             	movq	%r14, %rsi
ffffffff8010259d: e8 ae 7e 00 00       	callq	0xffffffff8010a450 <percpu_set_df_stack>
ffffffff801025a2: 48 8d 3d b7 71 02 00 	leaq	0x271b7(%rip), %rdi     # 0xffffffff80129760 <str_283>
ffffffff801025a9: e8 e2 81 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff801025ae: 4c 89 ff             	movq	%r15, %rdi
ffffffff801025b1: e8 5a 84 00 00       	callq	0xffffffff8010aa10 <kernel__drivers__serial__print_u64>
ffffffff801025b6: 48 8d 3d bd 71 02 00 	leaq	0x271bd(%rip), %rdi     # 0xffffffff8012977a <str_290>
ffffffff801025bd: e8 ce 81 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff801025c2: 4c 89 f7             	movq	%r14, %rdi
ffffffff801025c5: e8 46 84 00 00       	callq	0xffffffff8010aa10 <kernel__drivers__serial__print_u64>
ffffffff801025ca: 48 89 df             	movq	%rbx, %rdi
ffffffff801025cd: e8 be 81 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff801025d2: 48 8d 3d b7 71 02 00 	leaq	0x271b7(%rip), %rdi     # 0xffffffff80129790 <str_303>
ffffffff801025d9: e8 b2 81 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff801025de: e8 bd 3b 00 00       	callq	0xffffffff801061a0 <run_percpu_tests>
ffffffff801025e3: 48 8d 3d c6 71 02 00 	leaq	0x271c6(%rip), %rdi     # 0xffffffff801297b0 <str_311>
ffffffff801025ea: e8 a1 81 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff801025ef: e8 fc 28 00 00       	callq	0xffffffff80104ef0 <run_async_tests>
ffffffff801025f4: 48 8d 3d e5 71 02 00 	leaq	0x271e5(%rip), %rdi     # 0xffffffff801297e0 <str_319>
ffffffff801025fb: e8 90 81 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff80102600: e8 7b 16 00 00       	callq	0xffffffff80103c80 <run_preempt_tests>
ffffffff80102605: 48 8d 3d 04 72 02 00 	leaq	0x27204(%rip), %rdi     # 0xffffffff80129810 <str_326>
ffffffff8010260c: e8 7f 81 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff80102611: e8 da 6f 01 00       	callq	0xffffffff801195f0 <smp_release_aps>
ffffffff80102616: e8 c5 4c 00 00       	callq	0xffffffff801072e0 <sched_start>
ffffffff8010261b: 48 8d 3d 1e 72 02 00 	leaq	0x2721e(%rip), %rdi     # 0xffffffff80129840 <str_333>
ffffffff80102622: e8 69 81 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff80102627: 48 8d 3d 32 72 02 00 	leaq	0x27232(%rip), %rdi     # 0xffffffff80129860 <str_340>
ffffffff8010262e: e8 5d 81 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff80102633: e8 16 a3 01 00       	callq	0xffffffff8011c94e <get_sip_elf_addr>
ffffffff80102638: 48 89 c7             	movq	%rax, %rdi
ffffffff8010263b: e8 c0 14 01 00       	callq	0xffffffff80113b00 <elf_load_sip>
ffffffff80102640: 49 89 c4             	movq	%rax, %r12
ffffffff80102643: 48 8d 3d 36 72 02 00 	leaq	0x27236(%rip), %rdi     # 0xffffffff80129880 <str_349>
ffffffff8010264a: e8 41 81 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff8010264f: 4c 89 e7             	movq	%r12, %rdi
ffffffff80102652: e8 b9 83 00 00       	callq	0xffffffff8010aa10 <kernel__drivers__serial__print_u64>
ffffffff80102657: 48 89 df             	movq	%rbx, %rdi
ffffffff8010265a: e8 31 81 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff8010265f: e8 a8 9d 01 00       	callq	0xffffffff8011c40c <read_cr3>
ffffffff80102664: 48 89 c3             	movq	%rax, %rbx
ffffffff80102667: 31 ff                	xorl	%edi, %edi
ffffffff80102669: 48 89 c6             	movq	%rax, %rsi
ffffffff8010266c: e8 af 7d 00 00       	callq	0xffffffff8010a420 <percpu_set_kernel_cr3>
ffffffff80102671: e8 aa 70 00 00       	callq	0xffffffff80109720 <sys_alloc_kernel_stack>
ffffffff80102676: 48 89 85 78 ff ff ff 	movq	%rax, -0x88(%rbp)
ffffffff8010267d: 48 89 c7             	movq	%rax, %rdi
ffffffff80102680: e8 9b 6b 01 00       	callq	0xffffffff80119220 <tss_init_tss>
ffffffff80102685: bf 01 00 00 00       	movl	$0x1, %edi
ffffffff8010268a: 4c 89 fe             	movq	%r15, %rsi
ffffffff8010268d: e8 be 6b 01 00       	callq	0xffffffff80119250 <tss_set_ist>
ffffffff80102692: bf 02 00 00 00       	movl	$0x2, %edi
ffffffff80102697: 4c 89 f6             	movq	%r14, %rsi
ffffffff8010269a: e8 b1 6b 01 00       	callq	0xffffffff80119250 <tss_set_ist>
ffffffff8010269f: e8 8c 6b 01 00       	callq	0xffffffff80119230 <tss_get_tss_addr>
ffffffff801026a4: 48 89 c7             	movq	%rax, %rdi
ffffffff801026a7: e8 84 74 01 00       	callq	0xffffffff80119b30 <gdt_init_ring3>
ffffffff801026ac: e8 8f 74 01 00       	callq	0xffffffff80119b40 <gdt_load_ring3>
ffffffff801026b1: 48 8d 3d e8 71 02 00 	leaq	0x271e8(%rip), %rdi     # 0xffffffff801298a0 <str_372>
ffffffff801026b8: e8 d3 80 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff801026bd: e8 fe 75 01 00       	callq	0xffffffff80119cc0 <idt_init_ist_gates>
ffffffff801026c2: 48 8d 3d 17 72 02 00 	leaq	0x27217(%rip), %rdi     # 0xffffffff801298e0 <str_379>
ffffffff801026c9: e8 c2 80 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff801026ce: e8 4d 9d 01 00       	callq	0xffffffff8011c420 <enable_pcid>
ffffffff801026d3: 48 8d 3d 46 72 02 00 	leaq	0x27246(%rip), %rdi     # 0xffffffff80129920 <str_386>
ffffffff801026da: e8 b1 80 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff801026df: e8 98 9d 01 00       	callq	0xffffffff8011c47c <init_syscall_msrs>
ffffffff801026e4: 48 8b bd 78 ff ff ff 	movq	-0x88(%rbp), %rdi
ffffffff801026eb: e8 ca 9d 01 00       	callq	0xffffffff8011c4ba <set_syscall_kernel_rsp0>
ffffffff801026f0: 48 8d 3d 59 72 02 00 	leaq	0x27259(%rip), %rdi     # 0xffffffff80129950 <str_394>
ffffffff801026f7: e8 94 80 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff801026fc: 48 89 df             	movq	%rbx, %rdi
ffffffff801026ff: e8 ac 32 00 00       	callq	0xffffffff801059b0 <syscall_set_kernel_pml4>
ffffffff80102704: e8 1d a2 01 00       	callq	0xffffffff8011c926 <get_kernel_event_loop_addr>
ffffffff80102709: 48 89 c7             	movq	%rax, %rdi
ffffffff8010270c: 48 89 de             	movq	%rbx, %rsi
ffffffff8010270f: e8 2c 6d 00 00       	callq	0xffffffff80109440 <exec_spawn_kernel_thread>
ffffffff80102714: be 01 00 00 00       	movl	$0x1, %esi
ffffffff80102719: 48 89 c7             	movq	%rax, %rdi
ffffffff8010271c: e8 ef 74 00 00       	callq	0xffffffff80109c10 <process_set_state>
ffffffff80102721: 48 8d 3d 58 72 02 00 	leaq	0x27258(%rip), %rdi     # 0xffffffff80129980 <str_405>
ffffffff80102728: e8 63 80 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff8010272d: 48 89 df             	movq	%rbx, %rdi
ffffffff80102730: e8 4b d9 00 00       	callq	0xffffffff80110080 <sys_create_user_pml4_kpti>
ffffffff80102735: 49 89 c6             	movq	%rax, %r14
ffffffff80102738: 48 8d 3d 71 72 02 00 	leaq	0x27271(%rip), %rdi     # 0xffffffff801299b0 <str_413>
ffffffff8010273f: e8 4c 80 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff80102744: e8 c7 7e 00 00       	callq	0xffffffff8010a610 <fb_get_width>
ffffffff80102749: 89 c0                	movl	%eax, %eax
ffffffff8010274b: 48 89 45 c0          	movq	%rax, -0x40(%rbp)
ffffffff8010274f: e8 cc 7e 00 00       	callq	0xffffffff8010a620 <fb_get_height>
ffffffff80102754: 41 89 c4             	movl	%eax, %r12d
ffffffff80102757: e8 d4 7e 00 00       	callq	0xffffffff8010a630 <fb_get_pitch>
ffffffff8010275c: 41 89 c5             	movl	%eax, %r13d
ffffffff8010275f: e8 dc 7e 00 00       	callq	0xffffffff8010a640 <fb_get_phys>
ffffffff80102764: 49 89 c7             	movq	%rax, %r15
ffffffff80102767: 48 c7 45 a8 00 00 00 00      	movq	$0x0, -0x58(%rbp)
ffffffff8010276f: 48 81 7d a8 ff 03 00 00      	cmpq	$0x3ff, -0x58(%rbp) # imm = 0x3FF
ffffffff80102777: 77 35                	ja	0xffffffff801027ae <kmain+0x38e>
ffffffff80102779: 0f 1f 80 00 00 00 00 	nopl	(%rax)
ffffffff80102780: 48 8b 75 a8          	movq	-0x58(%rbp), %rsi
ffffffff80102784: 48 c1 e6 0c          	shlq	$0xc, %rsi
ffffffff80102788: 49 8d 14 37          	leaq	(%r15,%rsi), %rdx
ffffffff8010278c: 48 81 c6 00 20 00 40 	addq	$0x40002000, %rsi       # imm = 0x40002000
ffffffff80102793: b9 07 00 00 00       	movl	$0x7, %ecx
ffffffff80102798: 4c 89 f7             	movq	%r14, %rdi
ffffffff8010279b: e8 b0 d9 00 00       	callq	0xffffffff80110150 <map_user_page_extern>
ffffffff801027a0: 48 ff 45 a8          	incq	-0x58(%rbp)
ffffffff801027a4: 48 81 7d a8 ff 03 00 00      	cmpq	$0x3ff, -0x58(%rbp) # imm = 0x3FF
ffffffff801027ac: 76 d2                	jbe	0xffffffff80102780 <kmain+0x360>
ffffffff801027ae: e8 7d 25 00 00       	callq	0xffffffff80104d30 <pmm_alloc>
ffffffff801027b3: 49 89 c7             	movq	%rax, %r15
ffffffff801027b6: be 00 10 00 40       	movl	$0x40001000, %esi       # imm = 0x40001000
ffffffff801027bb: b9 07 00 00 00       	movl	$0x7, %ecx
ffffffff801027c0: 4c 89 f7             	movq	%r14, %rdi
ffffffff801027c3: 48 89 c2             	movq	%rax, %rdx
ffffffff801027c6: e8 85 d9 00 00       	callq	0xffffffff80110150 <map_user_page_extern>
ffffffff801027cb: 49 8d bf 00 00 00 80 	leaq	-0x80000000(%r15), %rdi
ffffffff801027d2: e8 49 95 00 00       	callq	0xffffffff8010bd20 <ps2_set_rx_ring>
ffffffff801027d7: 49 c7 87 00 00 00 80 00 00 00 00     	movq	$0x0, -0x80000000(%r15)
ffffffff801027e2: 49 c7 87 08 00 00 80 40 0f 00 00     	movq	$0xf40, -0x7ffffff8(%r15) # imm = 0xF40
ffffffff801027ed: 49 c7 87 40 00 00 80 00 00 00 00     	movq	$0x0, -0x7fffffc0(%r15)
ffffffff801027f8: 49 c7 87 80 00 00 80 00 00 00 00     	movq	$0x0, -0x7fffff80(%r15)
ffffffff80102803: e8 28 25 00 00       	callq	0xffffffff80104d30 <pmm_alloc>
ffffffff80102808: 49 89 c7             	movq	%rax, %r15
ffffffff8010280b: be 00 30 00 40       	movl	$0x40003000, %esi       # imm = 0x40003000
ffffffff80102810: b9 07 00 00 00       	movl	$0x7, %ecx
ffffffff80102815: 4c 89 f7             	movq	%r14, %rdi
ffffffff80102818: 48 89 c2             	movq	%rax, %rdx
ffffffff8010281b: e8 30 d9 00 00       	callq	0xffffffff80110150 <map_user_page_extern>
ffffffff80102820: 49 8d 87 00 00 00 80 	leaq	-0x80000000(%r15), %rax
ffffffff80102827: 49 c7 87 00 00 00 80 00 00 00 00     	movq	$0x0, -0x80000000(%r15)
ffffffff80102832: 49 c7 87 08 00 00 80 40 0f 00 00     	movq	$0xf40, -0x7ffffff8(%r15) # imm = 0xF40
ffffffff8010283d: 49 c7 87 40 00 00 80 00 00 00 00     	movq	$0x0, -0x7fffffc0(%r15)
ffffffff80102848: 49 c7 87 80 00 00 80 00 00 00 00     	movq	$0x0, -0x7fffff80(%r15)
ffffffff80102853: 48 89 05 f6 a8 04 00 	movq	%rax, 0x4a8f6(%rip)     # 0xffffffff8014d150 <kernel__core__main__KERNEL_TX_RING_VIRT>
ffffffff8010285a: 48 c7 45 b0 00 00 00 00      	movq	$0x0, -0x50(%rbp)
ffffffff80102862: 48 83 7d b0 0f       	cmpq	$0xf, -0x50(%rbp)
ffffffff80102867: 77 36                	ja	0xffffffff8010289f <kmain+0x47f>
ffffffff80102869: 0f 1f 80 00 00 00 00 	nopl	(%rax)
ffffffff80102870: e8 bb 24 00 00       	callq	0xffffffff80104d30 <pmm_alloc>
ffffffff80102875: 48 8b 75 b0          	movq	-0x50(%rbp), %rsi
ffffffff80102879: 48 c1 e6 0c          	shlq	$0xc, %rsi
ffffffff8010287d: 48 81 c6 00 20 40 40 	addq	$0x40402000, %rsi       # imm = 0x40402000
ffffffff80102884: b9 07 00 00 00       	movl	$0x7, %ecx
ffffffff80102889: 4c 89 f7             	movq	%r14, %rdi
ffffffff8010288c: 48 89 c2             	movq	%rax, %rdx
ffffffff8010288f: e8 bc d8 00 00       	callq	0xffffffff80110150 <map_user_page_extern>
ffffffff80102894: 48 ff 45 b0          	incq	-0x50(%rbp)
ffffffff80102898: 48 83 7d b0 0f       	cmpq	$0xf, -0x50(%rbp)
ffffffff8010289d: 76 d1                	jbe	0xffffffff80102870 <kmain+0x450>
ffffffff8010289f: 48 8d 3d ca f9 ff ff 	leaq	-0x636(%rip), %rdi      # 0xffffffff80102270 <kernel__core__main__sys_terminal_main>
ffffffff801028a6: 48 8b 75 c0          	movq	-0x40(%rbp), %rsi
ffffffff801028aa: 4c 89 e2             	movq	%r12, %rdx
ffffffff801028ad: 4c 89 e9             	movq	%r13, %rcx
ffffffff801028b0: 4d 89 f0             	movq	%r14, %r8
ffffffff801028b3: e8 a8 6b 00 00       	callq	0xffffffff80109460 <exec_spawn_ring3_coroutine>
ffffffff801028b8: be 01 00 00 00       	movl	$0x1, %esi
ffffffff801028bd: 48 89 c7             	movq	%rax, %rdi
ffffffff801028c0: e8 4b 73 00 00       	callq	0xffffffff80109c10 <process_set_state>
ffffffff801028c5: 48 8d 3d 24 71 02 00 	leaq	0x27124(%rip), %rdi     # 0xffffffff801299f0 <str_525>
ffffffff801028cc: e8 bf 7e 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff801028d1: 48 8d 3d c8 f9 ff ff 	leaq	-0x638(%rip), %rdi      # 0xffffffff801022a0 <terminal_tx_poll_thread>
ffffffff801028d8: 48 89 de             	movq	%rbx, %rsi
ffffffff801028db: e8 60 6b 00 00       	callq	0xffffffff80109440 <exec_spawn_kernel_thread>
ffffffff801028e0: be 01 00 00 00       	movl	$0x1, %esi
ffffffff801028e5: 48 89 c7             	movq	%rax, %rdi
ffffffff801028e8: e8 23 73 00 00       	callq	0xffffffff80109c10 <process_set_state>
ffffffff801028ed: 48 8d 3d 2c 71 02 00 	leaq	0x2712c(%rip), %rdi     # 0xffffffff80129a20 <str_538>
ffffffff801028f4: e8 97 7e 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff801028f9: 48 89 df             	movq	%rbx, %rdi
ffffffff801028fc: e8 7f d7 00 00       	callq	0xffffffff80110080 <sys_create_user_pml4_kpti>
ffffffff80102901: 49 89 c6             	movq	%rax, %r14
ffffffff80102904: 48 8d 3d 75 f9 ff ff 	leaq	-0x68b(%rip), %rdi      # 0xffffffff80102280 <kernel__core__main__sys_netd_main>
ffffffff8010290b: 31 f6                	xorl	%esi, %esi
ffffffff8010290d: 31 d2                	xorl	%edx, %edx
ffffffff8010290f: 31 c9                	xorl	%ecx, %ecx
ffffffff80102911: 49 89 c0             	movq	%rax, %r8
ffffffff80102914: e8 47 6b 00 00       	callq	0xffffffff80109460 <exec_spawn_ring3_coroutine>
ffffffff80102919: be 01 00 00 00       	movl	$0x1, %esi
ffffffff8010291e: 48 89 c7             	movq	%rax, %rdi
ffffffff80102921: e8 ea 72 00 00       	callq	0xffffffff80109c10 <process_set_state>
ffffffff80102926: 48 8d 3d 23 71 02 00 	leaq	0x27123(%rip), %rdi     # 0xffffffff80129a50 <str_558>
ffffffff8010292d: e8 5e 7e 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff80102932: 48 8d 3d 57 71 02 00 	leaq	0x27157(%rip), %rdi     # 0xffffffff80129a90 <str_565>
ffffffff80102939: e8 52 7e 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff8010293e: e8 5d 84 00 00       	callq	0xffffffff8010ada0 <moe_net_init>
ffffffff80102943: e8 68 84 00 00       	callq	0xffffffff8010adb0 <moe_enable_rx_interrupts>
ffffffff80102948: bf 05 00 00 00       	movl	$0x5, %edi
ffffffff8010294d: e8 3e 87 00 00       	callq	0xffffffff8010b090 <virtio_net_get_mac_byte>
ffffffff80102952: 41 89 c7             	movl	%eax, %r15d
ffffffff80102955: 48 8d 3d 54 71 02 00 	leaq	0x27154(%rip), %rdi     # 0xffffffff80129ab0 <str_576>
ffffffff8010295c: e8 2f 7e 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff80102961: 45 0f b6 ff          	movzbl	%r15b, %r15d
ffffffff80102965: 4c 89 ff             	movq	%r15, %rdi
ffffffff80102968: e8 a3 80 00 00       	callq	0xffffffff8010aa10 <kernel__drivers__serial__print_u64>
ffffffff8010296d: 48 8d 3d 16 6d 02 00 	leaq	0x26d16(%rip), %rdi     # 0xffffffff8012968a <str_199>
ffffffff80102974: e8 17 7e 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff80102979: e8 b2 23 00 00       	callq	0xffffffff80104d30 <pmm_alloc>
ffffffff8010297e: 49 89 c4             	movq	%rax, %r12
ffffffff80102981: 48 05 80 00 00 80    	addq	$-0x7fffff80, %rax      # imm = 0x80000080
ffffffff80102987: 48 89 45 c0          	movq	%rax, -0x40(%rbp)
ffffffff8010298b: c7 45 d4 00 00 00 00 	movl	$0x0, -0x2c(%rbp)
ffffffff80102992: 41 80 ff bb          	cmpb	$-0x45, %r15b
ffffffff80102996: 75 32                	jne	0xffffffff801029ca <kmain+0x5aa>
ffffffff80102998: 48 8d 3d 31 71 02 00 	leaq	0x27131(%rip), %rdi     # 0xffffffff80129ad0 <str_606>
ffffffff8010299f: e8 ec 7d 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff801029a4: c7 45 d4 02 00 00 00 	movl	$0x2, -0x2c(%rbp)
ffffffff801029ab: bf 52 00 00 00       	movl	$0x52, %edi
ffffffff801029b0: be 54 00 00 00       	movl	$0x54, %esi
ffffffff801029b5: 31 d2                	xorl	%edx, %edx
ffffffff801029b7: b9 12 00 00 00       	movl	$0x12, %ecx
ffffffff801029bc: 41 b8 34 00 00 00    	movl	$0x34, %r8d
ffffffff801029c2: 41 b9 aa 00 00 00    	movl	$0xaa, %r9d
ffffffff801029c8: eb 30                	jmp	0xffffffff801029fa <kmain+0x5da>
ffffffff801029ca: 48 8d 3d 2f 71 02 00 	leaq	0x2712f(%rip), %rdi     # 0xffffffff80129b00 <str_620>
ffffffff801029d1: e8 ba 7d 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff801029d6: c7 45 d4 00 00 00 00 	movl	$0x0, -0x2c(%rbp)
ffffffff801029dd: bf 52 00 00 00       	movl	$0x52, %edi
ffffffff801029e2: be 54 00 00 00       	movl	$0x54, %esi
ffffffff801029e7: 31 d2                	xorl	%edx, %edx
ffffffff801029e9: b9 12 00 00 00       	movl	$0x12, %ecx
ffffffff801029ee: 41 b8 34 00 00 00    	movl	$0x34, %r8d
ffffffff801029f4: 41 b9 bb 00 00 00    	movl	$0xbb, %r9d
ffffffff801029fa: e8 91 ea ff ff       	callq	0xffffffff80101490 <syscall_set_moe_peer_mac>
ffffffff801029ff: 49 8d 84 24 00 00 00 80      	leaq	-0x80000000(%r12), %rax
ffffffff80102a07: 48 89 45 90          	movq	%rax, -0x70(%rbp)
ffffffff80102a0b: 4d 89 e5             	movq	%r12, %r13
ffffffff80102a0e: 49 81 c5 00 01 00 80 	addq	$-0x7fffff00, %r13      # imm = 0x80000100
ffffffff80102a15: 8b 45 d4             	movl	-0x2c(%rbp), %eax
ffffffff80102a18: 41 89 84 24 00 00 00 80      	movl	%eax, -0x80000000(%r12)
ffffffff80102a20: 49 8d 84 24 00 02 00 80      	leaq	-0x7ffffe00(%r12), %rax
ffffffff80102a28: 49 89 84 24 18 00 00 80      	movq	%rax, -0x7fffffe8(%r12)
ffffffff80102a30: 49 8d bc 24 80 00 00 80      	leaq	-0x7fffff80(%r12), %rdi
ffffffff80102a38: e8 23 ea ff ff       	callq	0xffffffff80101460 <moe_set_bar_ptr>
ffffffff80102a3d: e8 ee 22 00 00       	callq	0xffffffff80104d30 <pmm_alloc>
ffffffff80102a42: 49 89 c7             	movq	%rax, %r15
ffffffff80102a45: 48 8d b8 00 00 00 80 	leaq	-0x80000000(%rax), %rdi
ffffffff80102a4c: 48 c7 80 00 00 00 80 00 00 00 00     	movq	$0x0, -0x80000000(%rax)
ffffffff80102a57: 48 c7 80 08 00 00 80 40 0f 00 00     	movq	$0xf40, -0x7ffffff8(%rax) # imm = 0xF40
ffffffff80102a62: 48 c7 80 40 00 00 80 00 00 00 00     	movq	$0x0, -0x7fffffc0(%rax)
ffffffff80102a6d: 48 c7 80 80 00 00 80 00 00 00 00     	movq	$0x0, -0x7fffff80(%rax)
ffffffff80102a78: e8 83 e9 00 00       	callq	0xffffffff80111400 <netcore_set_rx_notify>
ffffffff80102a7d: 48 c7 45 98 00 00 42 00      	movq	$0x420000, -0x68(%rbp) # imm = 0x420000
ffffffff80102a85: 48 c7 45 b8 00 00 00 40      	movq	$0x40000000, -0x48(%rbp) # imm = 0x40000000
ffffffff80102a8d: 48 c7 45 80 18 00 00 00      	movq	$0x18, -0x80(%rbp)
ffffffff80102a95: 48 c7 45 c8 00 00 00 00      	movq	$0x0, -0x38(%rbp)
ffffffff80102a9d: 48 89 df             	movq	%rbx, %rdi
ffffffff80102aa0: e8 db d5 00 00       	callq	0xffffffff80110080 <sys_create_user_pml4_kpti>
ffffffff80102aa5: 48 89 c3             	movq	%rax, %rbx
ffffffff80102aa8: 0f 1f 84 00 00 00 00 00      	nopl	(%rax,%rax)
ffffffff80102ab0: 48 8b 45 c8          	movq	-0x38(%rbp), %rax
ffffffff80102ab4: 48 3b 45 80          	cmpq	-0x80(%rbp), %rax
ffffffff80102ab8: 73 46                	jae	0xffffffff80102b00 <kmain+0x6e0>
ffffffff80102aba: 48 8b 55 c8          	movq	-0x38(%rbp), %rdx
ffffffff80102abe: 48 c1 e2 0c          	shlq	$0xc, %rdx
ffffffff80102ac2: 48 8b 75 b8          	movq	-0x48(%rbp), %rsi
ffffffff80102ac6: 48 01 d6             	addq	%rdx, %rsi
ffffffff80102ac9: 48 03 55 98          	addq	-0x68(%rbp), %rdx
ffffffff80102acd: b9 07 02 00 00       	movl	$0x207, %ecx            # imm = 0x207
ffffffff80102ad2: 4c 89 f7             	movq	%r14, %rdi
ffffffff80102ad5: e8 76 d6 00 00       	callq	0xffffffff80110150 <map_user_page_extern>
ffffffff80102ada: 48 8b 55 c8          	movq	-0x38(%rbp), %rdx
ffffffff80102ade: 48 c1 e2 0c          	shlq	$0xc, %rdx
ffffffff80102ae2: 48 8b 75 b8          	movq	-0x48(%rbp), %rsi
ffffffff80102ae6: 48 01 d6             	addq	%rdx, %rsi
ffffffff80102ae9: 48 03 55 98          	addq	-0x68(%rbp), %rdx
ffffffff80102aed: b9 07 02 00 00       	movl	$0x207, %ecx            # imm = 0x207
ffffffff80102af2: 48 89 df             	movq	%rbx, %rdi
ffffffff80102af5: e8 56 d6 00 00       	callq	0xffffffff80110150 <map_user_page_extern>
ffffffff80102afa: 48 ff 45 c8          	incq	-0x38(%rbp)
ffffffff80102afe: eb b0                	jmp	0xffffffff80102ab0 <kmain+0x690>
ffffffff80102b00: 48 8d 3d 29 70 02 00 	leaq	0x27029(%rip), %rdi     # 0xffffffff80129b30 <str_698>
ffffffff80102b07: e8 84 7c 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff80102b0c: 48 c7 45 a0 00 40 00 40      	movq	$0x40004000, -0x60(%rbp) # imm = 0x40004000
ffffffff80102b14: be 00 40 00 40       	movl	$0x40004000, %esi       # imm = 0x40004000
ffffffff80102b19: b9 07 02 00 00       	movl	$0x207, %ecx            # imm = 0x207
ffffffff80102b1e: 4c 89 f7             	movq	%r14, %rdi
ffffffff80102b21: 4c 89 fa             	movq	%r15, %rdx
ffffffff80102b24: e8 27 d6 00 00       	callq	0xffffffff80110150 <map_user_page_extern>
ffffffff80102b29: 48 8b 75 a0          	movq	-0x60(%rbp), %rsi
ffffffff80102b2d: b9 07 02 00 00       	movl	$0x207, %ecx            # imm = 0x207
ffffffff80102b32: 48 89 df             	movq	%rbx, %rdi
ffffffff80102b35: 4c 89 fa             	movq	%r15, %rdx
ffffffff80102b38: e8 13 d6 00 00       	callq	0xffffffff80110150 <map_user_page_extern>
ffffffff80102b3d: 48 8b 45 a0          	movq	-0x60(%rbp), %rax
ffffffff80102b41: 49 89 84 24 48 00 00 80      	movq	%rax, -0x7fffffb8(%r12)
ffffffff80102b49: 48 8b 45 b8          	movq	-0x48(%rbp), %rax
ffffffff80102b4d: 49 89 84 24 50 00 00 80      	movq	%rax, -0x7fffffb0(%r12)
ffffffff80102b55: 48 8d 3d c4 89 01 00 	leaq	0x189c4(%rip), %rdi     # 0xffffffff8011b520 <sys_moe_init_trampoline>
ffffffff80102b5c: 48 8b 75 90          	movq	-0x70(%rbp), %rsi
ffffffff80102b60: 48 8b 55 c0          	movq	-0x40(%rbp), %rdx
ffffffff80102b64: 4c 89 e9             	movq	%r13, %rcx
ffffffff80102b67: 49 89 d8             	movq	%rbx, %r8
ffffffff80102b6a: e8 f1 68 00 00       	callq	0xffffffff80109460 <exec_spawn_ring3_coroutine>
ffffffff80102b6f: 49 89 c6             	movq	%rax, %r14
ffffffff80102b72: 48 8d 3d f7 6f 02 00 	leaq	0x26ff7(%rip), %rdi     # 0xffffffff80129b70 <str_724>
ffffffff80102b79: e8 12 7c 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff80102b7e: 4c 89 f7             	movq	%r14, %rdi
ffffffff80102b81: e8 9a 70 00 00       	callq	0xffffffff80109c20 <process_set_current>
ffffffff80102b86: be 02 00 00 00       	movl	$0x2, %esi
ffffffff80102b8b: 4c 89 f7             	movq	%r14, %rdi
ffffffff80102b8e: e8 7d 70 00 00       	callq	0xffffffff80109c10 <process_set_state>
ffffffff80102b93: 4c 89 f7             	movq	%r14, %rdi
ffffffff80102b96: e8 95 70 00 00       	callq	0xffffffff80109c30 <process_get_kernel_rsp>
ffffffff80102b9b: 48 89 c3             	movq	%rax, %rbx
ffffffff80102b9e: 4c 89 f7             	movq	%r14, %rdi
ffffffff80102ba1: e8 9a 70 00 00       	callq	0xffffffff80109c40 <process_get_pml4>
ffffffff80102ba6: 49 89 c7             	movq	%rax, %r15
ffffffff80102ba9: 4c 89 f7             	movq	%r14, %rdi
ffffffff80102bac: e8 9f 70 00 00       	callq	0xffffffff80109c50 <process_get_pcid>
ffffffff80102bb1: 49 09 c7             	orq	%rax, %r15
ffffffff80102bb4: 4c 89 f7             	movq	%r14, %rdi
ffffffff80102bb7: e8 a4 70 00 00       	callq	0xffffffff80109c60 <process_get_kernel_stack_top>
ffffffff80102bbc: 49 89 c6             	movq	%rax, %r14
ffffffff80102bbf: 48 89 c7             	movq	%rax, %rdi
ffffffff80102bc2: e8 79 66 01 00       	callq	0xffffffff80119240 <tss_set_rsp0>
ffffffff80102bc7: 4c 89 f7             	movq	%r14, %rdi
ffffffff80102bca: e8 eb 98 01 00       	callq	0xffffffff8011c4ba <set_syscall_kernel_rsp0>
ffffffff80102bcf: 48 c7 45 88 00 00 00 00      	movq	$0x0, -0x78(%rbp)
ffffffff80102bd7: 48 8d 3d b2 6f 02 00 	leaq	0x26fb2(%rip), %rdi     # 0xffffffff80129b90 <str_739>
ffffffff80102bde: e8 ad 7b 00 00       	callq	0xffffffff8010a790 <kernel__drivers__serial__print>
ffffffff80102be3: 48 8b 7d 88          	movq	-0x78(%rbp), %rdi
ffffffff80102be7: 48 89 de             	movq	%rbx, %rsi
ffffffff80102bea: 4c 89 fa             	movq	%r15, %rdx
ffffffff80102bed: 4c 89 f1             	movq	%r14, %rcx
ffffffff80102bf0: e8 7c 16 02 00       	callq	0xffffffff80124271 <proc_context_switch>
ffffffff80102bf5: 66 66 2e 0f 1f 84 00 00 00 00 00     	nopw	%cs:(%rax,%rax)
ffffffff80102c00: e8 cf 9c 01 00       	callq	0xffffffff8011c8d4 <idle_halt>
ffffffff80102c05: eb f9                	jmp	0xffffffff80102c00 <kmain+0x7e0>
ffffffff80102c07: 66 0f 1f 84 00 00 00 00 00   	nopw	(%rax,%rax)

ffffffff80102c10 <kernel__core__main__strcmp>:
