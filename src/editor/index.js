/**
 * RustPress Editor — WordPress-style TinyMCE Rich Text Editor
 *
 * Full-screen editor matching the WordPress admin UI:
 * - No admin sidebar / admin bar (full-screen mode default)
 * - Header (64px): Back, +Inserter, Undo/Redo | Save Draft, Publish, Settings gear
 * - Left: Block Inserter panel (350px, toggle)
 * - Center: Gray chrome + white canvas (840px) with TinyMCE rich text editor
 * - Right: Settings sidebar (280px) with Post/Block tabs
 * - Visual/Text tab toggle (like WordPress Classic Editor)
 */

import apiFetch from '@wordpress/api-fetch';
import { createElement, useState, useEffect, useCallback, useRef } from '@wordpress/element';
import { createRoot } from '@wordpress/element';
import {
	BlockEditorProvider,
	BlockList,
	BlockInspector,
	WritingFlow,
	ObserveTyping,
	BlockToolbar,
	BlockTools,
} from '@wordpress/block-editor';
import {
	registerBlockType,
	createBlock,
	serialize,
	parse,
	setDefaultBlockName,
	setFreeformContentHandlerName,
	getBlockTypes,
} from '@wordpress/blocks';
import {
	SlotFillProvider,
	Popover,
} from '@wordpress/components';
import {
	ShortcutProvider,
} from '@wordpress/keyboard-shortcuts';
import { useDispatch, useSelect } from '@wordpress/data';
import { store as blockEditorStore } from '@wordpress/block-editor';
import { RichText, useBlockProps } from '@wordpress/block-editor';

// Styles
import '@wordpress/block-editor/build-style/style.css';
import '@wordpress/components/build-style/style.css';

// ============================================================
// Image Upload Helper
// ============================================================

async function uploadMediaFile(file) {
	const formData = new FormData();
	formData.append('file', file);
	const res = await fetch('/admin/media', {
		method: 'POST',
		body: formData,
		credentials: 'same-origin',
	});
	if (!res.ok) {
		const text = await res.text();
		throw new Error(text || res.statusText);
	}
	return res.json();
}

// ============================================================
// Media Library Modal
// ============================================================

function MediaLibraryModal({ onSelect, onClose }) {
	const [items, setItems] = useState([]);
	const [loading, setLoading] = useState(true);

	useEffect(() => {
		fetch('/admin/media?per_page=50&mime_type=image', { credentials: 'same-origin' })
			.then(r => r.json())
			.then(data => { setItems(data.items || []); setLoading(false); })
			.catch(() => setLoading(false));
	}, []);

	return createElement('div', { className: 'rp-media-modal-overlay', onClick: (e) => { if (e.target === e.currentTarget) onClose(); } },
		createElement('div', { className: 'rp-media-modal' },
			createElement('div', { className: 'rp-media-modal__header' },
				createElement('span', { style: { fontWeight: 600, fontSize: 14 } }, 'Media Library'),
				createElement('button', { className: 'rp-media-modal__close', onClick: onClose }, '\u00d7'),
			),
			createElement('div', { className: 'rp-media-modal__body' },
				loading
					? createElement('div', { style: { textAlign: 'center', padding: 32, color: '#757575' } }, 'Loading...')
					: items.length === 0
						? createElement('div', { style: { textAlign: 'center', padding: 32, color: '#757575' } }, 'No images found. Upload one first.')
						: createElement('div', { className: 'rp-media-grid' },
							items.map(item => createElement('button', {
								key: item.id,
								className: 'rp-media-grid__item',
								onClick: () => onSelect(item),
								title: item.title,
							},
								createElement('img', { src: item.url, alt: item.title }),
							)),
						),
			),
		),
	);
}

// ============================================================
// Image Block Edit Component
// ============================================================

function ImageBlockEdit({ attributes, setAttributes }) {
	const [uploading, setUploading] = useState(false);
	const [error, setError] = useState(null);
	const [showLibrary, setShowLibrary] = useState(false);
	const [dragOver, setDragOver] = useState(false);
	const fileInputRef = useRef(null);

	const handleFile = useCallback(async (file) => {
		if (!file || !file.type.startsWith('image/')) {
			setError('Please select an image file.');
			return;
		}
		setError(null);
		setUploading(true);
		try {
			const media = await uploadMediaFile(file);
			setAttributes({ url: media.url, id: media.id, alt: media.title || '' });
		} catch (err) {
			setError('Upload failed: ' + err.message);
		}
		setUploading(false);
	}, [setAttributes]);

	const handleDrop = useCallback((e) => {
		e.preventDefault();
		e.stopPropagation();
		setDragOver(false);
		const file = e.dataTransfer?.files?.[0];
		if (file) handleFile(file);
	}, [handleFile]);

	const handleDragOver = useCallback((e) => {
		e.preventDefault();
		e.stopPropagation();
		setDragOver(true);
	}, []);

	const handleDragLeave = useCallback((e) => {
		e.preventDefault();
		setDragOver(false);
	}, []);

	// Placeholder state: no image yet
	if (!attributes.url) {
		return createElement('div', useBlockProps(),
			createElement('div', {
				className: 'rp-image-placeholder' + (dragOver ? ' is-drag-over' : ''),
				onDrop: handleDrop,
				onDragOver: handleDragOver,
				onDragLeave: handleDragLeave,
			},
				uploading
					? createElement('div', { className: 'rp-image-placeholder__uploading' },
						createElement('div', { className: 'rp-image-spinner' }),
						createElement('span', null, 'Uploading...'),
					)
					: createElement('div', { className: 'rp-image-placeholder__inner' },
						createElement('div', { className: 'rp-image-placeholder__icon' }, '\ud83d\uddbc'),
						createElement('div', { className: 'rp-image-placeholder__buttons' },
							createElement('button', {
								className: 'rp-image-btn rp-image-btn--primary',
								onClick: () => fileInputRef.current?.click(),
							}, 'Upload'),
							createElement('button', {
								className: 'rp-image-btn',
								onClick: () => setShowLibrary(true),
							}, 'Media Library'),
						),
						createElement('p', { className: 'rp-image-placeholder__hint' }, 'Drop an image here, or click to upload'),
						createElement('div', { className: 'rp-image-placeholder__url-row' },
							createElement('input', {
								type: 'text',
								placeholder: 'Paste image URL',
								className: 'rp-image-url-input',
								onKeyDown: (e) => {
									if (e.key === 'Enter' && e.target.value.trim()) {
										setAttributes({ url: e.target.value.trim() });
									}
								},
							}),
						),
					),
				error && createElement('div', { className: 'rp-image-error' }, error),
			),
			createElement('input', {
				ref: fileInputRef,
				type: 'file',
				accept: 'image/*',
				style: { display: 'none' },
				onChange: (e) => { const f = e.target.files?.[0]; if (f) handleFile(f); e.target.value = ''; },
			}),
			showLibrary && createElement(MediaLibraryModal, {
				onSelect: (item) => { setAttributes({ url: item.url, id: item.id, alt: item.title || '' }); setShowLibrary(false); },
				onClose: () => setShowLibrary(false),
			}),
		);
	}

	// Image is set: show the image with replace controls
	return createElement('figure', useBlockProps(),
		createElement('div', { style: { position: 'relative' } },
			createElement('img', { src: attributes.url, alt: attributes.alt, style: { maxWidth: '100%', height: 'auto', display: 'block' } }),
			createElement('div', { className: 'rp-image-toolbar' },
				createElement('button', {
					className: 'rp-image-toolbar__btn',
					onClick: () => fileInputRef.current?.click(),
					title: 'Replace',
				}, 'Replace'),
				createElement('button', {
					className: 'rp-image-toolbar__btn',
					onClick: () => setAttributes({ url: '', id: undefined, alt: '' }),
					title: 'Remove',
				}, '\u00d7'),
			),
		),
		createElement('div', { style: { marginTop: 8 } },
			createElement('input', {
				type: 'text',
				value: attributes.alt || '',
				onChange: (e) => setAttributes({ alt: e.target.value }),
				placeholder: 'Alt text (describe the image)',
				style: { width: '100%', padding: '4px 8px', border: '1px solid #ccc', borderRadius: 2, fontSize: 12, color: '#757575' },
			}),
		),
		createElement(RichText, { tagName: 'figcaption', value: attributes.caption, onChange: (c) => setAttributes({ caption: c }), placeholder: 'Add caption...', style: { fontSize: '0.85em', color: '#757575', textAlign: 'center', marginTop: 8 } }),
		createElement('input', {
			ref: fileInputRef,
			type: 'file',
			accept: 'image/*',
			style: { display: 'none' },
			onChange: async (e) => {
				const f = e.target.files?.[0];
				if (f) {
					setUploading(true);
					try { const m = await uploadMediaFile(f); setAttributes({ url: m.url, id: m.id, alt: m.title || '' }); } catch(_){}
					setUploading(false);
				}
				e.target.value = '';
			},
		}),
	);
}

// ============================================================
// Block Registration
// ============================================================

function registerAllBlocks() {
	const blocks = [
		{ name: 'core/paragraph', title: 'Paragraph', cat: 'text', icon: '\u00b6',
		  desc: 'Start with the basic building block of all narrative.',
		  attrs: { content: { type: 'string', source: 'html', selector: 'p', default: '' }, dropCap: { type: 'boolean', default: false }, placeholder: { type: 'string' }, align: { type: 'string' } },
		  supports: { anchor: true, className: true, color: { gradients: true, link: true }, spacing: { margin: true, padding: true }, typography: { fontSize: true, lineHeight: true }, __unstablePasteTextInline: true },
		  edit( { attributes, setAttributes } ) {
			return createElement( RichText, { ...useBlockProps(), tagName: 'p', value: attributes.content, onChange: ( c ) => setAttributes( { content: c } ), placeholder: attributes.placeholder || 'Type / to choose a block' } );
		  },
		  save( { attributes } ) { return createElement( RichText.Content, { tagName: 'p', value: attributes.content } ); },
		},
		{ name: 'core/heading', title: 'Heading', cat: 'text', icon: 'H',
		  desc: 'Introduce new sections and organize content.',
		  attrs: { content: { type: 'string', source: 'html', selector: 'h1,h2,h3,h4,h5,h6', default: '' }, level: { type: 'number', default: 2 } },
		  supports: { anchor: true, className: true, color: { gradients: true, link: true }, typography: { fontSize: true } },
		  edit( { attributes, setAttributes } ) {
			const tag = 'h' + attributes.level;
			return createElement( 'div', useBlockProps(),
				createElement( 'div', { style: { marginBottom: 8, display: 'flex', gap: 4 } },
					...[2,3,4].map(l => createElement('button', { key:l, onClick:()=>setAttributes({level:l}), style:{ padding:'2px 8px', fontSize:12, cursor:'pointer', background: attributes.level===l?'#1e1e1e':'#f0f0f0', color: attributes.level===l?'#fff':'#1e1e1e', border:'1px solid #ccc', borderRadius:2 } }, 'H'+l))
				),
				createElement( RichText, { tagName: tag, value: attributes.content, onChange: (c) => setAttributes({content:c}), placeholder: 'Heading' } ),
			);
		  },
		  save( { attributes } ) { return createElement( RichText.Content, { tagName: 'h'+attributes.level, value: attributes.content } ); },
		},
		{ name: 'core/list', title: 'List', cat: 'text', icon: '\u2630',
		  desc: 'Create a bulleted or numbered list.',
		  attrs: { ordered: { type: 'boolean', default: false }, values: { type: 'string', source: 'html', selector: 'ol,ul', multiline: 'li', default: '' } },
		  supports: { anchor: true, className: true },
		  edit( { attributes, setAttributes } ) {
			return createElement( 'div', useBlockProps(),
				createElement( 'div', { style:{marginBottom:8,display:'flex',gap:4} },
					createElement('button',{onClick:()=>setAttributes({ordered:false}),style:{padding:'2px 8px',fontSize:12,background:!attributes.ordered?'#1e1e1e':'#f0f0f0',color:!attributes.ordered?'#fff':'#1e1e1e',border:'1px solid #ccc',borderRadius:2,cursor:'pointer'}},'• Unordered'),
					createElement('button',{onClick:()=>setAttributes({ordered:true}),style:{padding:'2px 8px',fontSize:12,background:attributes.ordered?'#1e1e1e':'#f0f0f0',color:attributes.ordered?'#fff':'#1e1e1e',border:'1px solid #ccc',borderRadius:2,cursor:'pointer'}},'1. Ordered'),
				),
				createElement( RichText, { tagName: attributes.ordered?'ol':'ul', multiline:'li', value: attributes.values, onChange:(v)=>setAttributes({values:v}), placeholder:'List' } ),
			);
		  },
		  save( { attributes } ) { return createElement( RichText.Content, { tagName: attributes.ordered?'ol':'ul', multiline:'li', value: attributes.values } ); },
		},
		{ name: 'core/quote', title: 'Quote', cat: 'text', icon: '\u201c',
		  desc: 'Give quoted text visual emphasis.',
		  attrs: { value: { type:'string',source:'html',selector:'blockquote > p',default:'' }, citation: { type:'string',source:'html',selector:'cite',default:'' } },
		  edit({attributes,setAttributes}){
			return createElement('blockquote',useBlockProps(),
				createElement(RichText,{tagName:'p',value:attributes.value,onChange:(v)=>setAttributes({value:v}),placeholder:'Write quote...'}),
				createElement(RichText,{tagName:'cite',value:attributes.citation,onChange:(c)=>setAttributes({citation:c}),placeholder:'Write citation...',style:{fontSize:'0.85em',color:'#757575'}}),
			);
		  },
		  save({attributes}){ return createElement('blockquote',null,createElement(RichText.Content,{tagName:'p',value:attributes.value}),attributes.citation&&createElement(RichText.Content,{tagName:'cite',value:attributes.citation})); },
		},
		{ name: 'core/code', title: 'Code', cat: 'text', icon: '</>',
		  desc: 'Display code snippets.',
		  attrs: { content: { type:'string',source:'html',selector:'code',default:'' } },
		  edit({attributes,setAttributes}){
			return createElement('pre',useBlockProps(),createElement(RichText,{tagName:'code',value:attributes.content,onChange:(c)=>setAttributes({content:c}),placeholder:'Write code...',preserveWhiteSpace:true}));
		  },
		  save({attributes}){ return createElement('pre',null,createElement(RichText.Content,{tagName:'code',value:attributes.content})); },
		},
		{ name: 'core/image', title: 'Image', cat: 'media', icon: '\ud83d\uddbc',
		  desc: 'Insert an image.',
		  attrs: { url:{type:'string',source:'attribute',selector:'img',attribute:'src'}, alt:{type:'string',source:'attribute',selector:'img',attribute:'alt',default:''}, caption:{type:'string',source:'html',selector:'figcaption',default:''}, id:{type:'number'} },
		  edit: ImageBlockEdit,
		  save({attributes}){ return createElement('figure',null,createElement('img',{src:attributes.url,alt:attributes.alt}),attributes.caption&&createElement(RichText.Content,{tagName:'figcaption',value:attributes.caption})); },
		},
		{ name:'core/separator', title:'Separator', cat:'design', icon:'\u2015', desc:'Create a break between ideas.',
		  edit(){ return createElement('hr',useBlockProps({style:{border:'none',borderTop:'2px solid #e0e0e0',margin:'28px 0'}})); },
		  save(){ return createElement('hr'); },
		},
		{ name:'core/spacer', title:'Spacer', cat:'design', icon:'\u2195', desc:'Add white space between blocks.',
		  attrs:{ height:{type:'string',default:'100px'} },
		  edit({attributes}){ return createElement('div',{...useBlockProps({style:{height:attributes.height,background:'repeating-linear-gradient(45deg,transparent,transparent 5px,rgba(0,0,0,0.03) 5px,rgba(0,0,0,0.03) 10px)',border:'1px dashed #ddd',position:'relative'}})}); },
		  save({attributes}){ return createElement('div',{style:{height:attributes.height},'aria-hidden':'true',className:'wp-block-spacer'}); },
		},
		{ name:'core/html', title:'Custom HTML', cat:'widgets', icon:'<>', desc:'Add custom HTML code.',
		  attrs:{ content:{type:'string',default:''} },
		  edit({attributes,setAttributes}){
			return createElement('div',useBlockProps(),
				createElement('label',{style:{fontSize:11,fontWeight:600,display:'block',marginBottom:4,color:'#757575',textTransform:'uppercase',letterSpacing:'0.5px'}},'HTML'),
				createElement('textarea',{value:attributes.content,onChange:(e)=>setAttributes({content:e.target.value}),placeholder:'<p>Enter HTML here...</p>',rows:6,style:{width:'100%',fontFamily:'Menlo,Consolas,monospace',fontSize:13,padding:8,border:'1px solid #ccc',borderRadius:2,background:'#f9f9f9'}}),
			);
		  },
		  save({attributes}){ return createElement(RichText.Content,{value:attributes.content}); },
		},
		{ name:'core/preformatted', title:'Preformatted', cat:'text', icon:'pre', desc:'Text that respects spacing and tabs.',
		  attrs:{ content:{type:'string',source:'html',selector:'pre',default:''} },
		  edit({attributes,setAttributes}){
			return createElement(RichText,{...useBlockProps(),tagName:'pre',value:attributes.content,onChange:(c)=>setAttributes({content:c}),placeholder:'Write preformatted text...',preserveWhiteSpace:true});
		  },
		  save({attributes}){ return createElement(RichText.Content,{tagName:'pre',value:attributes.content}); },
		},
		{ name:'core/freeform', title:'Classic', cat:'text', icon:'\u270d', desc:'Use the classic editor.',
		  attrs:{ content:{type:'string',source:'raw',default:''} },
		  edit({attributes,setAttributes}){
			return createElement('div',useBlockProps(),createElement('textarea',{value:attributes.content,onChange:(e)=>setAttributes({content:e.target.value}),placeholder:'Classic editor content...',rows:8,style:{width:'100%',fontFamily:'inherit',fontSize:14,padding:12,border:'1px solid #ddd',borderRadius:2}}));
		  },
		  save({attributes}){ return createElement(RichText.Content,{value:attributes.content}); },
		},
		{ name:'core/buttons', title:'Buttons', cat:'design', icon:'\u25a3', desc:'Button-style links.' },
		{ name:'core/columns', title:'Columns', cat:'design', icon:'\u2261', desc:'Display content in columns.' },
		{ name:'core/group', title:'Group', cat:'design', icon:'\u25a1', desc:'Gather blocks in a container.' },
		{ name:'core/table', title:'Table', cat:'text', icon:'\u2637', desc:'Structured content in rows and columns.' },
	];

	blocks.forEach( b => {
		const def = {
			apiVersion: 3,
			title: b.title,
			category: b.cat,
			description: b.desc,
			attributes: b.attrs || {},
			supports: b.supports || { anchor: true, className: true },
			edit: b.edit || function(){ return createElement('div',useBlockProps({style:{padding:16,border:'1px dashed #ccc',borderRadius:2,textAlign:'center',color:'#757575',fontSize:13}}), b.title + ' Block'); },
			save: b.save || function(){ return null; },
		};
		registerBlockType( b.name, def );
	});

	setDefaultBlockName('core/paragraph');
	setFreeformContentHandlerName('core/freeform');
}

// ============================================================
// API setup
// ============================================================

apiFetch.use( apiFetch.createRootURLMiddleware( window.rpEditorSettings?.apiRoot || '/wp-json/' ) );
apiFetch.use( ( opts, next ) => { opts.credentials = 'same-origin'; return next(opts); } );

// ============================================================
// Block Inserter Panel (Fallback — always works)
// ============================================================

function BlockInserterPanel({ onClose }) {
	const [search, setSearch] = useState('');
	const { insertBlock: storeInsertBlock } = useDispatch(blockEditorStore);
	const allBlocks = getBlockTypes();
	const categories = [
		{ slug:'text', label:'Text' },
		{ slug:'media', label:'Media' },
		{ slug:'design', label:'Design' },
		{ slug:'widgets', label:'Widgets' },
	];

	const filtered = search
		? allBlocks.filter(b => b.title.toLowerCase().includes(search.toLowerCase()))
		: allBlocks;

	const handleInsert = useCallback((name) => {
		const block = createBlock(name);
		storeInsertBlock(block);
	}, [storeInsertBlock]);

	return createElement('div', { className: 'editor-inserter-panel' },
		createElement('div', { className: 'editor-inserter-panel__header' },
			createElement('span', null, 'Blocks'),
			createElement('button', { className: 'editor-inserter-panel__close', onClick: onClose, 'aria-label':'Close' }, '\u00d7'),
		),
		createElement('div', { className: 'editor-inserter-panel__body' },
			createElement('input', {
				className: 'inserter-search',
				type: 'text',
				placeholder: 'Search',
				value: search,
				onChange: (e) => setSearch(e.target.value),
				autoFocus: true,
			}),
			categories.map(cat => {
				const catBlocks = filtered.filter(b => b.category === cat.slug);
				if (!catBlocks.length) return null;
				return createElement('div', { key: cat.slug, className: 'inserter-category' },
					createElement('div', { className: 'inserter-category__title' }, cat.label),
					createElement('div', { className: 'inserter-blocks' },
						catBlocks.map(b => createElement('button', {
							key: b.name,
							className: 'inserter-block',
							onClick: () => handleInsert(b.name),
							title: b.description || b.title,
						},
							createElement('span', { className: 'inserter-block__icon' }, getBlockIcon(b.name)),
							createElement('span', { className: 'inserter-block__name' }, b.title),
						)),
					),
				);
			}),
		),
	);
}

function getBlockIcon(name) {
	const map = {
		'core/paragraph': '\u00b6',
		'core/heading': 'H',
		'core/list': '\u2630',
		'core/quote': '\u201c',
		'core/code': '</>',
		'core/image': '\ud83d\uddbc',
		'core/separator': '\u2015',
		'core/spacer': '\u2195',
		'core/html': '<>',
		'core/preformatted': 'pre',
		'core/freeform': '\u270d',
		'core/buttons': '\u25a3',
		'core/columns': '\u2261',
		'core/group': '\u25a1',
		'core/table': '\u2637',
	};
	return map[name] || '\u25a0';
}

// ============================================================
// Custom Fields Metabox
// ============================================================

function CustomFieldsMetabox({ postId }) {
	const [fields, setFields] = useState([]);
	const [collapsed, setCollapsed] = useState(false);
	const [loading, setLoading] = useState(false);
	const [existingKeys, setExistingKeys] = useState([]);
	const [newKeyMode, setNewKeyMode] = useState('select'); // 'select' | 'input'
	const [newKey, setNewKey] = useState('');
	const [newValue, setNewValue] = useState('');
	const [editingId, setEditingId] = useState(null);
	const [editKey, setEditKey] = useState('');
	const [editValue, setEditValue] = useState('');
	const [status, setStatus] = useState(null); // { type: 'saving'|'error'|'saved', msg: string }

	// Load custom fields from server or initial data
	useEffect(() => {
		if (!postId) return;
		// Check if initial data was passed through settings
		const settings = window.rpEditorSettings || {};
		if (settings.customFields && settings.customFields.length >= 0) {
			setFields(settings.customFields);
		}
		// Also load from API for freshness
		loadFields();
		loadMetaKeys();
	}, [postId]);

	const loadFields = useCallback(() => {
		if (!postId) return;
		setLoading(true);
		fetch('/admin/posts/' + postId + '/meta', { credentials: 'same-origin' })
			.then(r => { if (!r.ok) throw new Error('Failed'); return r.json(); })
			.then(data => { setFields(data); setLoading(false); })
			.catch(() => setLoading(false));
	}, [postId]);

	const loadMetaKeys = useCallback(() => {
		fetch('/admin/posts/meta-keys', { credentials: 'same-origin' })
			.then(r => r.json())
			.then(keys => setExistingKeys(keys || []))
			.catch(() => {});
	}, []);

	const showStatus = useCallback((type, msg) => {
		setStatus({ type, msg });
		if (type !== 'error') {
			setTimeout(() => setStatus(null), 2000);
		}
	}, []);

	const addField = useCallback(async () => {
		const key = newKey.trim();
		if (!key || !postId) return;
		showStatus('saving', 'Saving...');
		try {
			const res = await fetch('/admin/posts/' + postId + '/meta', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				credentials: 'same-origin',
				body: JSON.stringify({ key, value: newValue }),
			});
			if (!res.ok) {
				const text = await res.text();
				throw new Error(text || 'Failed to add');
			}
			const created = await res.json();
			setFields(prev => [...prev, created]);
			setNewKey('');
			setNewValue('');
			// Add the key to existing keys if not present
			setExistingKeys(prev => {
				if (!prev.includes(key)) return [...prev, key].sort();
				return prev;
			});
			showStatus('saved', 'Added!');
		} catch (e) {
			showStatus('error', 'Error: ' + e.message);
		}
	}, [postId, newKey, newValue, showStatus]);

	const startEdit = useCallback((field) => {
		setEditingId(field.meta_id);
		setEditKey(field.key);
		setEditValue(field.value);
	}, []);

	const cancelEdit = useCallback(() => {
		setEditingId(null);
		setEditKey('');
		setEditValue('');
	}, []);

	const saveEdit = useCallback(async (metaId) => {
		if (!postId) return;
		showStatus('saving', 'Saving...');
		try {
			const res = await fetch('/admin/posts/' + postId + '/meta/' + metaId, {
				method: 'PUT',
				headers: { 'Content-Type': 'application/json' },
				credentials: 'same-origin',
				body: JSON.stringify({ key: editKey.trim() || undefined, value: editValue }),
			});
			if (!res.ok) {
				const text = await res.text();
				throw new Error(text || 'Failed to update');
			}
			const updated = await res.json();
			setFields(prev => prev.map(f => f.meta_id === metaId ? updated : f));
			setEditingId(null);
			showStatus('saved', 'Updated!');
		} catch (e) {
			showStatus('error', 'Error: ' + e.message);
		}
	}, [postId, editKey, editValue, showStatus]);

	const deleteField = useCallback(async (metaId) => {
		if (!postId) return;
		if (!window.confirm('Delete this custom field?')) return;
		showStatus('saving', 'Deleting...');
		try {
			const res = await fetch('/admin/posts/' + postId + '/meta/' + metaId, {
				method: 'DELETE',
				credentials: 'same-origin',
			});
			if (!res.ok && res.status !== 204) {
				throw new Error('Failed to delete');
			}
			setFields(prev => prev.filter(f => f.meta_id !== metaId));
			if (editingId === metaId) cancelEdit();
			showStatus('saved', 'Deleted!');
		} catch (e) {
			showStatus('error', 'Error: ' + e.message);
		}
	}, [postId, editingId, cancelEdit, showStatus]);

	if (!postId) return null;

	// Build dropdown options: existing keys minus keys already used on this post
	const usedKeys = fields.map(f => f.key);
	const availableKeys = existingKeys.filter(k => !usedKeys.includes(k));

	// -- Table rows for existing fields --
	const fieldRows = fields.map(field => {
		if (editingId === field.meta_id) {
			// Edit mode
			return createElement('tr', { key: field.meta_id },
				createElement('td', { className: 'rp-cf-key' },
					createElement('input', {
						className: 'rp-cf-key-input',
						type: 'text',
						value: editKey,
						onChange: (e) => setEditKey(e.target.value),
					}),
				),
				createElement('td', { className: 'rp-cf-value' },
					createElement('textarea', {
						className: 'rp-cf-value-textarea',
						value: editValue,
						onChange: (e) => setEditValue(e.target.value),
					}),
				),
				createElement('td', { className: 'rp-cf-actions' },
					createElement('button', {
						className: 'rp-cf-btn rp-cf-btn--primary rp-cf-btn--small',
						onClick: () => saveEdit(field.meta_id),
						style: { marginRight: 4 },
					}, 'Save'),
					createElement('button', {
						className: 'rp-cf-btn rp-cf-btn--small',
						onClick: cancelEdit,
					}, 'Cancel'),
				),
			);
		}

		// Display mode
		return createElement('tr', { key: field.meta_id },
			createElement('td', { className: 'rp-cf-key' },
				createElement('strong', null, field.key),
			),
			createElement('td', { className: 'rp-cf-value' },
				createElement('div', { className: 'rp-cf-value-display' }, field.value || '(empty)'),
			),
			createElement('td', { className: 'rp-cf-actions' },
				createElement('button', {
					className: 'rp-cf-btn rp-cf-btn--small',
					onClick: () => startEdit(field),
					style: { marginRight: 4 },
				}, 'Edit'),
				createElement('button', {
					className: 'rp-cf-btn rp-cf-btn--danger rp-cf-btn--small',
					onClick: () => deleteField(field.meta_id),
				}, 'Delete'),
			),
		);
	});

	// -- Status indicator --
	const statusEl = status && createElement('span', {
		className: 'rp-cf-status' + (status.type === 'saving' ? ' rp-cf-status--saving' : '') + (status.type === 'error' ? ' rp-cf-status--error' : ''),
	}, status.msg);

	return createElement('div', { className: 'rp-custom-fields' },
		// Header (collapsible)
		createElement('div', {
			className: 'rp-custom-fields__header',
			onClick: () => setCollapsed(!collapsed),
		},
			createElement('span', { className: 'rp-custom-fields__title' },
				'Custom Fields',
				statusEl,
			),
			createElement('span', {
				className: 'rp-custom-fields__toggle' + (collapsed ? ' is-collapsed' : ''),
			}, '\u25BC'),
		),
		// Body
		createElement('div', {
			className: 'rp-custom-fields__body' + (collapsed ? ' is-collapsed' : ''),
		},
			// Table of existing fields
			fields.length > 0
				? createElement('table', { className: 'rp-custom-fields__table' },
					createElement('thead', null,
						createElement('tr', null,
							createElement('th', { className: 'rp-cf-key' }, 'Name'),
							createElement('th', { className: 'rp-cf-value' }, 'Value'),
							createElement('th', { className: 'rp-cf-actions' }, ''),
						),
					),
					createElement('tbody', null, ...fieldRows),
				)
				: !loading && createElement('div', { className: 'rp-cf-empty' },
					'No custom fields yet. Add one below.',
				),
			loading && fields.length === 0 && createElement('div', { className: 'rp-cf-empty' },
				'Loading custom fields...',
			),
			// Add new field row
			createElement('div', { className: 'rp-cf-add-row' },
				createElement('div', { className: 'rp-cf-add-row__title' }, 'Add New Custom Field'),
				createElement('div', { className: 'rp-cf-add-row__fields' },
					createElement('div', { className: 'rp-cf-add-row__key-wrap' },
						// Dropdown of existing keys + input toggle
						availableKeys.length > 0 && newKeyMode === 'select'
							? createElement('div', null,
								createElement('select', {
									className: 'rp-cf-key-select',
									value: newKey,
									onChange: (e) => setNewKey(e.target.value),
								},
									createElement('option', { value: '' }, '-- Select --'),
									availableKeys.map(k =>
										createElement('option', { key: k, value: k }, k),
									),
								),
								createElement('button', {
									className: 'rp-cf-btn rp-cf-btn--small',
									onClick: () => { setNewKeyMode('input'); setNewKey(''); },
									style: { marginTop: 4, fontSize: 11 },
								}, 'Enter new'),
							)
							: createElement('div', null,
								createElement('input', {
									className: 'rp-cf-key-input',
									type: 'text',
									placeholder: 'Key',
									value: newKey,
									onChange: (e) => setNewKey(e.target.value),
									onKeyDown: (e) => { if (e.key === 'Enter') { e.preventDefault(); addField(); } },
								}),
								availableKeys.length > 0 && createElement('button', {
									className: 'rp-cf-btn rp-cf-btn--small',
									onClick: () => { setNewKeyMode('select'); setNewKey(''); },
									style: { marginTop: 4, fontSize: 11 },
								}, 'Select existing'),
							),
					),
					createElement('div', { className: 'rp-cf-add-row__value-wrap' },
						createElement('textarea', {
							className: 'rp-cf-value-textarea',
							placeholder: 'Value',
							value: newValue,
							onChange: (e) => setNewValue(e.target.value),
							rows: 2,
						}),
					),
					createElement('div', { className: 'rp-cf-add-row__btn-wrap' },
						createElement('button', {
							className: 'rp-cf-btn rp-cf-btn--primary',
							onClick: addField,
							disabled: !newKey.trim(),
						}, 'Add Custom Field'),
					),
				),
			),
		),
	);
}

// ============================================================
// PostEditor — Main Component
// ============================================================

function PostEditor() {
	const settings = window.rpEditorSettings || {};
	const postId = settings.postId || 0;
	const postType = settings.postType || 'post';

	const [blocks, setBlocks] = useState([]);
	const [title, setTitle] = useState(settings.postTitle || '');
	const [status, setStatus] = useState(settings.postStatus || 'draft');
	const [excerpt, setExcerpt] = useState(settings.postExcerpt || '');
	const [slug, setSlug] = useState(settings.postSlug || '');
	const [saving, setSaving] = useState(false);
	const [notice, setNotice] = useState(null);
	const [inserterOpen, setInserterOpen] = useState(false);
	const [sidebarOpen, setSidebarOpen] = useState(true);
	const [sidebarTab, setSidebarTab] = useState('post'); // 'post' | 'block'
	const [currentPostId, setCurrentPostId] = useState(postId);

	// TinyMCE state
	const [editorMode, setEditorMode] = useState('visual'); // 'visual' | 'text'
	const [htmlContent, setHtmlContent] = useState(settings.postContent || '');
	const tinymceInitialized = useRef(false);
	const textareaRef = useRef(null);

	// Featured Image state
	const [featuredImageId, setFeaturedImageId] = useState(settings.featuredImageId || 0);
	const [featuredImageUrl, setFeaturedImageUrl] = useState(settings.featuredImageUrl || '');
	const [showFeaturedMediaLib, setShowFeaturedMediaLib] = useState(false);
	const featuredFileRef = useRef(null);

	// Scheduled post date
	const [scheduledDate, setScheduledDate] = useState(settings.postDate || '');

	// Autosave state
	const [dirty, setDirty] = useState(false);
	const autosaveTimer = useRef(null);

	// Taxonomy state
	const [allCategories, setAllCategories] = useState([]);
	const [allTags, setAllTags] = useState([]);
	const [selectedCatIds, setSelectedCatIds] = useState([]);
	const [selectedTagIds, setSelectedTagIds] = useState([]);
	const [newTagName, setNewTagName] = useState('');

	// Author, Discussion, Sticky state
	const [postAuthor, setPostAuthor] = useState(settings.postAuthor || 1);
	const [allUsers, setAllUsers] = useState([]);
	const [commentStatus, setCommentStatus] = useState(settings.commentStatus || 'open');
	const [pingStatus, setPingStatus] = useState(settings.pingStatus || 'open');
	const [stickyPost, setStickyPost] = useState(settings.sticky || false);

	// Load users for author selector
	useEffect(() => {
		fetch('/admin/users?per_page=100', { credentials: 'same-origin' })
			.then(r => r.json())
			.then(data => setAllUsers(data.items || data || []))
			.catch(() => {});
	}, []);

	// Load categories and tags
	useEffect(() => {
		if (postType === 'post') {
			fetch('/admin/terms?taxonomy=category&per_page=200', { credentials: 'same-origin' })
				.then(r => r.json())
				.then(data => setAllCategories(data.items || []))
				.catch(() => {});
			fetch('/admin/terms?taxonomy=post_tag&per_page=200', { credentials: 'same-origin' })
				.then(r => r.json())
				.then(data => setAllTags(data.items || []))
				.catch(() => {});
			if (postId) {
				fetch('/admin/posts/' + postId + '/terms', { credentials: 'same-origin' })
					.then(r => r.json())
					.then(data => {
						if (data.categories) setSelectedCatIds(data.categories.map(c => c.term_id));
						if (data.tags) setSelectedTagIds(data.tags.map(t => t.term_id));
					})
					.catch(() => {});
			}
		}
	}, []);

	useEffect(() => {
		if (settings.postContent) {
			try {
				const parsed = parse(settings.postContent);
				setBlocks(parsed.length ? parsed : [createBlock('core/paragraph')]);
			} catch(e) {
				setBlocks([createBlock('core/paragraph')]);
			}
		} else {
			setBlocks([createBlock('core/paragraph')]);
		}
	}, []);

	// Mark dirty on content changes
	const handleBlocksChange = useCallback((newBlocks) => {
		setBlocks(newBlocks);
		setDirty(true);
	}, []);

	// Initialize TinyMCE
	useEffect(() => {
		if (tinymceInitialized.current) return;
		if (typeof window.tinymce === 'undefined') {
			console.warn('RustPress: TinyMCE not loaded from CDN, falling back to plain textarea');
			return;
		}

		const initTinyMCE = () => {
			const targetEl = document.getElementById('editor');
			if (!targetEl) {
				// Retry if DOM not ready
				setTimeout(initTinyMCE, 100);
				return;
			}
			tinymceInitialized.current = true;
			window.tinymce.init({
				selector: '#editor',
				base_url: '/static/vendor/tinymce',
				suffix: '.min',
				height: 500,
				min_height: 400,
				menubar: 'file edit view insert format tools table',
				plugins: [
					'advlist', 'autolink', 'lists', 'link', 'image', 'charmap',
					'preview', 'anchor', 'searchreplace', 'visualblocks', 'code',
					'fullscreen', 'insertdatetime', 'media', 'table', 'help',
					'wordcount', 'quickbars'
				],
				toolbar: 'formatselect | bold italic underline strikethrough | forecolor | ' +
					'bullist numlist | blockquote | ' +
					'alignleft aligncenter alignright | ' +
					'link unlink image media | code fullscreen',
				toolbar_mode: 'wrap',
				block_formats: 'Paragraph=p; Heading 2=h2; Heading 3=h3; Heading 4=h4; Heading 5=h5; Heading 6=h6; Preformatted=pre',
				content_style: `
					body {
						font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Oxygen-Sans, Ubuntu, Cantarell, "Helvetica Neue", sans-serif;
						font-size: 16px;
						line-height: 1.8;
						color: #1e1e1e;
						max-width: 100%;
						padding: 12px 16px;
					}
					p { margin: 0 0 1em 0; }
					img { max-width: 100%; height: auto; }
					blockquote {
						border-left: 4px solid #1e1e1e;
						padding: 0 24px;
						margin: 1em 0;
						color: #555;
					}
					pre {
						background: #f0f0f0;
						padding: 16px;
						border-radius: 2px;
						font-family: Menlo, Consolas, monaco, monospace;
						font-size: 0.875rem;
						overflow-x: auto;
					}
					a { color: #2271b1; }
				`,
				skin: 'oxide',
				promotion: false,
				branding: false,
				resize: true,
				elementpath: true,
				statusbar: true,
				convert_urls: false,
				relative_urls: false,
				remove_script_host: false,
				entity_encoding: 'raw',
				setup: function(ed) {
					ed.on('init', function() {
						ed.setContent(settings.postContent || '');
					});
					ed.on('change keyup input NodeChange', function() {
						setDirty(true);
					});
				},
				images_upload_handler: function(blobInfo, progress) {
					return new Promise(function(resolve, reject) {
						const formData = new FormData();
						formData.append('file', blobInfo.blob(), blobInfo.filename());
						fetch('/admin/media', {
							method: 'POST',
							body: formData,
							credentials: 'same-origin',
						}).then(function(res) {
							if (!res.ok) throw new Error('Upload failed');
							return res.json();
						}).then(function(data) {
							resolve(data.url);
						}).catch(function(err) {
							reject('Image upload failed: ' + err.message);
						});
					});
				},
			});
		};

		// Wait a tick for the DOM to render the textarea
		setTimeout(initTinyMCE, 50);

		return () => {
			if (window.tinymce && window.tinymce.get('editor')) {
				window.tinymce.get('editor').remove();
				tinymceInitialized.current = false;
			}
		};
	}, []);

	// Helper: get content from the active editor mode
	const getEditorContent = useCallback(() => {
		if (editorMode === 'visual' && window.tinymce && window.tinymce.get('editor')) {
			return window.tinymce.get('editor').getContent();
		}
		return htmlContent;
	}, [editorMode, htmlContent]);

	// Handle switching between Visual and Text modes
	const switchToVisual = useCallback(() => {
		if (editorMode === 'visual') return;
		const ed = window.tinymce && window.tinymce.get('editor');
		if (ed) {
			// Transfer content from text textarea to TinyMCE
			ed.setContent(htmlContent);
			// Show the TinyMCE container
			const container = ed.getContainer();
			if (container) container.style.display = '';
		}
		setEditorMode('visual');
	}, [editorMode, htmlContent]);

	const switchToText = useCallback(() => {
		if (editorMode === 'text') return;
		const ed = window.tinymce && window.tinymce.get('editor');
		if (ed) {
			// Transfer content from TinyMCE to text textarea
			const content = ed.getContent();
			setHtmlContent(content);
			// Hide the TinyMCE container
			const container = ed.getContainer();
			if (container) container.style.display = 'none';
		}
		setEditorMode('text');
	}, [editorMode]);

	// Autosave: every 60 seconds when dirty and we have a post ID
	useEffect(() => {
		if (autosaveTimer.current) clearInterval(autosaveTimer.current);
		autosaveTimer.current = setInterval(() => {
			if (dirty && currentPostId && !saving) {
				const content = getEditorContent();
				fetch(`/wp-json/wp/v2/posts/${currentPostId}`, {
					method: 'PUT',
					headers: { 'Content-Type': 'application/json' },
					credentials: 'same-origin',
					body: JSON.stringify({ title, content, excerpt }),
				}).then(() => {
					setDirty(false);
				}).catch(() => {});
			}
		}, 60000);
		return () => { if (autosaveTimer.current) clearInterval(autosaveTimer.current); };
	}, [dirty, currentPostId, saving, title, excerpt, getEditorContent]);

	// Featured image upload handler
	const handleFeaturedFile = useCallback(async (file) => {
		if (!file || !file.type.startsWith('image/')) return;
		try {
			const media = await uploadMediaFile(file);
			setFeaturedImageId(media.id);
			setFeaturedImageUrl(media.url);
			setDirty(true);
		} catch(e) {}
	}, []);

	const savePost = useCallback(async (newStatus) => {
		setSaving(true);
		setNotice(null);
		const s = newStatus || status;
		const content = getEditorContent();
		const payload = { title, content, excerpt, status: s, post_type: postType, featured_media: featuredImageId || 0, author: postAuthor, comment_status: commentStatus, ping_status: pingStatus, sticky: stickyPost };
		// Add scheduled date for future posts
		if (s === 'future' && scheduledDate) {
			payload.date = scheduledDate + ':00';
			payload.status = 'future';
		}
		try {
			let result;
			if (currentPostId) {
				result = await apiFetch({ path: `/wp-json/wp/v2/posts/${currentPostId}`, method:'PUT', data:payload });
			} else {
				result = await apiFetch({ path:'/wp-json/wp/v2/posts', method:'POST', data:payload });
				if (result && result.id) {
					setCurrentPostId(result.id);
					window.history.replaceState({}, '', `/wp-admin/post.php?post=${result.id}&action=edit`);
				}
			}
			// Save taxonomy assignments
			const pid = currentPostId || (result && result.id);
			if (pid && postType === 'post') {
				await fetch('/admin/posts/' + pid + '/terms', {
					method: 'PUT',
					headers: { 'Content-Type': 'application/json' },
					credentials: 'same-origin',
					body: JSON.stringify({ taxonomy: 'category', term_ids: selectedCatIds }),
				});
				await fetch('/admin/posts/' + pid + '/terms', {
					method: 'PUT',
					headers: { 'Content-Type': 'application/json' },
					credentials: 'same-origin',
					body: JSON.stringify({ taxonomy: 'post_tag', term_ids: selectedTagIds }),
				});
			}
			setStatus(s);
			setDirty(false);
			const label = postType === 'page' ? 'Page' : 'Post';
			if (s === 'future') {
				setNotice({ type:'success', msg: label + ' scheduled!' });
			} else {
				setNotice({ type:'success', msg: s==='publish' ? label + ' published!' : 'Draft saved.' });
			}
		} catch(err) {
			setNotice({ type:'error', msg: `Save failed: ${err.message||err}` });
		}
		setSaving(false);
	}, [getEditorContent, title, excerpt, status, currentPostId, selectedCatIds, selectedTagIds, featuredImageId, scheduledDate, postAuthor, commentStatus, pingStatus, stickyPost]);

	const contentRef = useRef(null);
	const editorSettings = { hasFixedToolbar: true, focusMode: false };

	// Undo/Redo from block editor store
	const { hasUndo, hasRedo } = useSelect((select) => {
		const store = select(blockEditorStore);
		return {
			hasUndo: store.hasUndo ? store.hasUndo() : false,
			hasRedo: store.hasRedo ? store.hasRedo() : false,
		};
	}, []);
	const { undo: editorUndo, redo: editorRedo } = useDispatch(blockEditorStore);

	// --- HEADER ---
	const header = createElement('div', { className: 'editor-header' },
		createElement('div', { className: 'editor-header__left' },
			// Back
			createElement('a', { className: 'editor-header__back', href: postType === 'page' ? '/wp-admin/edit.php?post_type=page' : '/wp-admin/edit.php', title: 'Back to ' + (postType === 'page' ? 'pages' : 'posts') },
				createElement('span', null, '\u2190'),
			),
			createElement('div', { className: 'editor-header__sep' }),
			// Inserter toggle
			createElement('button', {
				className: 'editor-inserter-toggle' + (inserterOpen ? ' is-pressed' : ''),
				onClick: () => setInserterOpen(!inserterOpen),
				'aria-label': 'Toggle block inserter',
			}, '+'),
			createElement('div', { className: 'editor-header__sep' }),
			// Undo
			createElement('button', {
				className: 'editor-header__btn',
				onClick: () => editorUndo && editorUndo(),
				disabled: !hasUndo,
				title: 'Undo',
				'aria-label': 'Undo',
			}, '\u21A9'),
			// Redo
			createElement('button', {
				className: 'editor-header__btn',
				onClick: () => editorRedo && editorRedo(),
				disabled: !hasRedo,
				title: 'Redo',
				'aria-label': 'Redo',
			}, '\u21AA'),
			createElement('div', { className: 'editor-header__sep' }),
			// Editor mode indicator
			createElement('span', { style: { fontSize: 12, color: '#757575' } }, editorMode === 'visual' ? 'Visual Editor' : 'Text Editor'),
			dirty && createElement('span', { style: { fontSize: 11, color: '#b32d2e', marginLeft: 4 } }, 'Unsaved'),
		),
		createElement('div', { className: 'editor-header__right' },
			// Save Draft
			createElement('button', {
				className: 'editor-header__text-btn',
				onClick: () => savePost('draft'),
				disabled: saving,
			}, 'Save draft'),
			// Publish / Update
			createElement('button', {
				className: 'editor-publish-btn',
				onClick: () => savePost('publish'),
				disabled: saving,
			}, saving ? 'Saving\u2026' : (status === 'publish' ? 'Update' : status === 'future' ? 'Schedule' : 'Publish')),
			createElement('div', { className: 'editor-header__sep' }),
			// Settings toggle
			createElement('button', {
				className: 'editor-header__btn' + (sidebarOpen ? ' is-active' : ''),
				onClick: () => setSidebarOpen(!sidebarOpen),
				title: 'Settings',
				'aria-label': 'Toggle settings',
			}, '\u2699'),
		),
	);

	// --- NOTICE ---
	const noticeBar = notice && createElement('div', {
		className: 'editor-notice editor-notice--' + notice.type,
		onClick: () => setNotice(null),
	}, notice.msg, createElement('span', { className: 'editor-notice__dismiss' }, '\u00d7'));

	// --- Category checkbox handler ---
	const toggleCategory = useCallback((catId) => {
		setSelectedCatIds(prev =>
			prev.includes(catId) ? prev.filter(id => id !== catId) : [...prev, catId]
		);
	}, []);

	// --- Add new tag handler ---
	const addNewTag = useCallback(async () => {
		const name = newTagName.trim();
		if (!name) return;
		try {
			const res = await fetch('/admin/terms', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				credentials: 'same-origin',
				body: JSON.stringify({ name, taxonomy: 'post_tag' }),
			});
			if (!res.ok) throw new Error('Failed');
			const tag = await res.json();
			setAllTags(prev => [...prev, tag]);
			setSelectedTagIds(prev => [...prev, tag.term_id]);
			setNewTagName('');
		} catch(e) {}
	}, [newTagName]);

	// --- Tag toggle handler ---
	const toggleTag = useCallback((tagId) => {
		setSelectedTagIds(prev =>
			prev.includes(tagId) ? prev.filter(id => id !== tagId) : [...prev, tagId]
		);
	}, []);

	// --- SETTINGS SIDEBAR (RIGHT) ---
	const postTabLabel = postType === 'page' ? 'Page' : 'Post';
	const settingsSidebar = sidebarOpen && createElement('div', { className: 'editor-settings-sidebar' },
		createElement('div', { className: 'editor-settings-tabs' },
			createElement('button', { className: 'editor-settings-tab' + (sidebarTab === 'post' ? ' is-active' : ''), onClick: () => setSidebarTab('post') }, postTabLabel),
			createElement('button', { className: 'editor-settings-tab' + (sidebarTab === 'block' ? ' is-active' : ''), onClick: () => setSidebarTab('block') }, 'Block'),
		),
		createElement('div', { className: 'editor-settings-body' },
			sidebarTab === 'post'
				? createElement('div', null,
					// Summary
					createElement('div', { className: 'settings-panel' },
						createElement('div', { className: 'settings-panel__title' }, 'Summary'),
						createElement('label', null, 'Status'),
						createElement('select', { className: 'settings-select', value: status, onChange: (e) => setStatus(e.target.value) },
							createElement('option', { value: 'draft' }, 'Draft'),
							createElement('option', { value: 'publish' }, 'Published'),
							createElement('option', { value: 'pending' }, 'Pending Review'),
							createElement('option', { value: 'private' }, 'Private'),
							createElement('option', { value: 'future' }, 'Scheduled'),
						),
						// Scheduled date picker (shown when status is 'future')
						status === 'future' && createElement('div', { style: { marginTop: 8 } },
							createElement('label', null, 'Publish Date'),
							createElement('input', {
								className: 'settings-input',
								type: 'datetime-local',
								value: scheduledDate,
								onChange: (e) => setScheduledDate(e.target.value),
							}),
						),
					),
					// URL
					createElement('div', { className: 'settings-panel' },
						createElement('div', { className: 'settings-panel__title' }, 'URL'),
						createElement('input', { className: 'settings-input', type: 'text', value: slug, onChange: (e) => setSlug(e.target.value), placeholder: 'post-slug' }),
					),
					// Featured Image
					createElement('div', { className: 'settings-panel' },
						createElement('div', { className: 'settings-panel__title' }, 'Featured Image'),
						featuredImageUrl
							? createElement('div', null,
								createElement('img', { src: featuredImageUrl, alt: 'Featured image', style: { width: '100%', height: 'auto', borderRadius: 2, marginBottom: 8 } }),
								createElement('div', { style: { display: 'flex', gap: 4 } },
									createElement('button', {
										onClick: () => featuredFileRef.current?.click(),
										style: { flex: 1, padding: '4px 8px', border: '1px solid #ccc', borderRadius: 2, background: '#f0f0f0', cursor: 'pointer', fontSize: 12 },
									}, 'Replace'),
									createElement('button', {
										onClick: () => { setFeaturedImageId(0); setFeaturedImageUrl(''); setDirty(true); },
										style: { padding: '4px 8px', border: '1px solid #cc1818', borderRadius: 2, background: '#fff', color: '#cc1818', cursor: 'pointer', fontSize: 12 },
									}, 'Remove'),
								),
							)
							: createElement('div', null,
								createElement('div', { style: { display: 'flex', gap: 4 } },
									createElement('button', {
										onClick: () => featuredFileRef.current?.click(),
										style: { flex: 1, padding: '6px 8px', border: '1px solid #2271b1', borderRadius: 2, background: '#f0f7fc', color: '#2271b1', cursor: 'pointer', fontSize: 12 },
									}, 'Upload'),
									createElement('button', {
										onClick: () => setShowFeaturedMediaLib(true),
										style: { flex: 1, padding: '6px 8px', border: '1px solid #ccc', borderRadius: 2, background: '#f0f0f0', cursor: 'pointer', fontSize: 12 },
									}, 'Media Library'),
								),
							),
						createElement('input', {
							ref: featuredFileRef,
							type: 'file',
							accept: 'image/*',
							style: { display: 'none' },
							onChange: (e) => { const f = e.target.files?.[0]; if (f) handleFeaturedFile(f); e.target.value = ''; },
						}),
					),
					// Categories (only for posts)
					postType === 'post' && createElement('div', { className: 'settings-panel' },
						createElement('div', { className: 'settings-panel__title' }, 'Categories'),
						createElement('div', { style: { maxHeight: 160, overflowY: 'auto', border: '1px solid #ddd', borderRadius: 2, padding: 8, fontSize: 13 } },
							allCategories.length === 0
								? createElement('div', { style: { color: '#757575', fontSize: 12 } }, 'No categories')
								: allCategories.map(cat =>
									createElement('label', { key: cat.term_id, style: { display: 'flex', alignItems: 'center', gap: 6, padding: '2px 0', cursor: 'pointer' } },
										createElement('input', {
											type: 'checkbox',
											checked: selectedCatIds.includes(cat.term_id),
											onChange: () => toggleCategory(cat.term_id),
										}),
										cat.name,
									),
								),
						),
					),
					// Tags (only for posts)
					postType === 'post' && createElement('div', { className: 'settings-panel' },
						createElement('div', { className: 'settings-panel__title' }, 'Tags'),
						createElement('div', { style: { display: 'flex', flexWrap: 'wrap', gap: 4, marginBottom: 8 } },
							allTags.filter(t => selectedTagIds.includes(t.term_id)).map(tag =>
								createElement('span', {
									key: tag.term_id,
									style: { display: 'inline-flex', alignItems: 'center', gap: 4, padding: '2px 8px', background: '#e0e0e0', borderRadius: 12, fontSize: 12 },
								},
									tag.name,
									createElement('button', {
										onClick: () => toggleTag(tag.term_id),
										style: { background: 'none', border: 'none', cursor: 'pointer', fontSize: 14, lineHeight: 1, padding: 0, color: '#757575' },
									}, '\u00d7'),
								),
							),
						),
						createElement('div', { style: { display: 'flex', gap: 4 } },
							createElement('input', {
								className: 'settings-input',
								type: 'text',
								value: newTagName,
								onChange: (e) => setNewTagName(e.target.value),
								placeholder: 'Add new tag',
								onKeyDown: (e) => { if (e.key === 'Enter') { e.preventDefault(); addNewTag(); } },
								style: { flex: 1 },
							}),
							createElement('button', {
								onClick: addNewTag,
								style: { padding: '4px 10px', border: '1px solid #ccc', borderRadius: 2, background: '#f0f0f0', cursor: 'pointer', fontSize: 12 },
							}, 'Add'),
						),
					),
					// Author
					createElement('div', { className: 'settings-panel' },
						createElement('div', { className: 'settings-panel__title' }, 'Author'),
						createElement('select', {
							className: 'settings-select',
							value: postAuthor,
							onChange: (e) => { setPostAuthor(Number(e.target.value)); setDirty(true); },
						},
							allUsers.map(u => createElement('option', { key: u.id || u.ID, value: u.id || u.ID }, u.display_name || u.user_login || ('User ' + (u.id || u.ID)))),
						),
					),
					// Excerpt
					createElement('div', { className: 'settings-panel' },
						createElement('div', { className: 'settings-panel__title' }, 'Excerpt'),
						createElement('textarea', { className: 'settings-textarea', value: excerpt, onChange: (e) => setExcerpt(e.target.value), placeholder: 'Write an excerpt (optional)' }),
					),
					// Sticky (posts only)
					postType === 'post' && createElement('div', { className: 'settings-panel' },
						createElement('label', { style: { display: 'flex', alignItems: 'center', gap: 8, textTransform: 'none', fontWeight: 'normal', fontSize: 13, letterSpacing: 0 } },
							createElement('input', {
								type: 'checkbox',
								checked: stickyPost,
								onChange: (e) => { setStickyPost(e.target.checked); setDirty(true); },
							}),
							'Stick to the top of the blog',
						),
					),
					// Discussion
					createElement('div', { className: 'settings-panel' },
						createElement('div', { className: 'settings-panel__title' }, 'Discussion'),
						createElement('label', { style: { display: 'flex', alignItems: 'center', gap: 8, textTransform: 'none', fontWeight: 'normal', fontSize: 13, letterSpacing: 0, marginBottom: 6 } },
							createElement('input', {
								type: 'checkbox',
								checked: commentStatus === 'open',
								onChange: (e) => { setCommentStatus(e.target.checked ? 'open' : 'closed'); setDirty(true); },
							}),
							'Allow comments',
						),
						createElement('label', { style: { display: 'flex', alignItems: 'center', gap: 8, textTransform: 'none', fontWeight: 'normal', fontSize: 13, letterSpacing: 0 } },
							createElement('input', {
								type: 'checkbox',
								checked: pingStatus === 'open',
								onChange: (e) => { setPingStatus(e.target.checked ? 'open' : 'closed'); setDirty(true); },
							}),
							'Allow pingbacks & trackbacks',
						),
					),
				)
				: createElement('div', null,
					createElement(BlockInspector),
				),
		),
	);

	// --- TinyMCE Content Editor Area ---
	const tinymceEditor = createElement('div', null,
		// Visual / Text tabs (WordPress-style)
		createElement('div', { className: 'rp-editor-tabs' },
			createElement('button', {
				className: 'rp-editor-tab' + (editorMode === 'visual' ? ' is-active' : ''),
				onClick: switchToVisual,
				type: 'button',
			}, 'Visual'),
			createElement('button', {
				className: 'rp-editor-tab' + (editorMode === 'text' ? ' is-active' : ''),
				onClick: switchToText,
				type: 'button',
			}, 'Text'),
		),
		// Editor content wrap
		createElement('div', { className: 'rp-editor-content-wrap' },
			// TinyMCE textarea (always in DOM; TinyMCE wraps it in its own container)
			createElement('textarea', {
				id: 'editor',
				style: { width: '100%', minHeight: 400, visibility: 'hidden' },
				defaultValue: settings.postContent || '',
			}),
			// Plain text editor (shown in Text mode)
			createElement('textarea', {
				ref: textareaRef,
				className: 'rp-text-editor',
				value: htmlContent,
				onChange: (e) => { setHtmlContent(e.target.value); setDirty(true); },
				placeholder: 'Enter HTML content here...',
				style: { display: editorMode === 'text' ? 'block' : 'none' },
			}),
		),
	);

	// --- BODY (BlockEditorProvider wraps everything needing block context) ---
	const body = createElement(BlockEditorProvider,
		{ value: blocks, onInput: handleBlocksChange, onChange: handleBlocksChange, settings: editorSettings },
		createElement('div', { className: 'editor-body' },
			// Left: inserter
			inserterOpen && createElement(BlockInserterPanel, {
				onClose: () => setInserterOpen(false),
			}),
			// Center: canvas with TinyMCE editor
			createElement('div', { className: 'editor-canvas' },
				createElement('div', { className: 'editor-canvas__inner' },
					createElement('input', {
						className: 'editor-title-input',
						type: 'text',
						placeholder: 'Add title',
						value: title,
						onChange: (e) => setTitle(e.target.value),
					}),
					createElement('hr', { className: 'editor-title-sep' }),
					// TinyMCE editor replaces the block editor content area
					tinymceEditor,
					// Custom Fields metabox (below editor, only shown for saved posts)
					currentPostId > 0 && createElement(CustomFieldsMetabox, { postId: currentPostId }),
				),
			),
			// Right: settings
			settingsSidebar,
		),
		createElement(Popover.Slot),
		showFeaturedMediaLib && createElement(MediaLibraryModal, {
			onSelect: (item) => { setFeaturedImageId(item.id); setFeaturedImageUrl(item.url); setShowFeaturedMediaLib(false); setDirty(true); },
			onClose: () => setShowFeaturedMediaLib(false),
		}),
	);

	return createElement(ShortcutProvider, null,
		createElement(SlotFillProvider, null,
			createElement('div', { className: 'editor-shell' },
				header,
				noticeBar,
				body,
			),
		),
	);
}

// ============================================================
// Init
// ============================================================

document.addEventListener('DOMContentLoaded', () => {
	try {
		registerAllBlocks();
		console.log('RustPress: Registered', getBlockTypes().length, 'block types');
	} catch(e) {
		console.error('RustPress: Block registration failed', e);
	}

	const rootEl = document.getElementById('rustpress-editor');
	if (rootEl) {
		const root = createRoot(rootEl);
		root.render(createElement(PostEditor));
	}
});
